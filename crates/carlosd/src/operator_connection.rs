//! Drive one operator action with the shared pure client and a blocking, deadline-bounded Unix socket.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use carlos_client::codec::{Framer, Incoming};
use carlos_client::{Client, ConnectionId, Outbound, ReadReply};
use carlos_proto::commands::Command;
use carlos_proto::devices::{DeviceAdd, DeviceInfo, DeviceRevoke, Devices};
use carlos_proto::ids::DeviceId;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};

pub(super) enum Action {
    Add {
        device: DeviceInfo,
        verifier: String,
    },
    List,
    Revoke(DeviceId),
}

pub(super) enum Reply {
    Devices(Devices),
    Operation(OperationResult),
}

struct Exchange {
    client: Client,
    connection: ConnectionId,
    socket: UnixStream,
    framer: Framer,
    operation: Option<ClientOperation>,
    started: bool,
    deadline: Instant,
}

pub(super) fn run(path: &Path, action: Action) -> Result<Reply> {
    Exchange::connect(path)?.run(&action)
}

impl Exchange {
    fn connect(path: &Path) -> Result<Self> {
        let socket = UnixStream::connect(path).context("connect to the local daemon")?;
        carlosd::socket::verify_peer(&socket).context("verify the daemon's OS user")?;
        socket.set_write_timeout(Some(Duration::from_millis(500)))?;
        let mut client = Client::default();
        let hello = client.begin("carlosd operator".into())?;
        let mut exchange = Self {
            connection: hello.connection(),
            client,
            socket,
            framer: Framer::default(),
            operation: None,
            started: false,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        exchange.send(hello)?;
        Ok(exchange)
    }

    fn run(&mut self, action: &Action) -> Result<Reply> {
        loop {
            while let Some(message) = self.framer.next_message()? {
                if let Some(reply) = self.apply(message)? {
                    return Ok(reply);
                }
            }
            if !self.started && self.client.is_ready(self.connection) {
                let (outgoing, operation) = action.start(&mut self.client, self.connection)?;
                self.operation = operation;
                self.started = true;
                self.send(outgoing)?;
            }
            if let Some(reply) = self.finished() {
                return Ok(Reply::Operation(reply));
            }
            self.receive()?;
        }
    }

    fn apply(&mut self, message: Incoming) -> Result<Option<Reply>> {
        let effects = self.client.receive(self.connection, message)?;
        if let Some(failure) = effects.failures.first() {
            bail!("{}", failure.error.message);
        }
        for outgoing in effects.outgoing {
            self.send(outgoing)?;
        }
        for result in effects.replies {
            if let ReadReply::Devices(devices) = result.reply {
                return Ok(Some(Reply::Devices(devices)));
            }
        }
        Ok(None)
    }

    fn finished(&self) -> Option<OperationResult> {
        let record = self.client.operations().get(self.operation.as_ref()?)?;
        if matches!(
            record.state,
            OperationState::Pending | OperationState::Queued
        ) {
            return None;
        }
        Some(OperationResult {
            operation: record.operation.clone(),
            session: record.session.clone(),
            rewound: record.rewound,
            state: record.state.clone(),
        })
    }

    fn send(&mut self, outgoing: Outbound) -> Result<()> {
        if self.socket.write_all(outgoing.bytes()).is_err() {
            self.client.write_failed(
                self.connection,
                outgoing.id(),
                "operator write was not confirmed",
                true,
            );
            bail!("operator write was not confirmed; no retry was sent");
        }
        Ok(())
    }

    fn receive(&mut self) -> Result<()> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("device request was not confirmed within30 seconds; no retry was sent");
        }
        self.socket.set_read_timeout(Some(remaining))?;
        let mut buffer = [0; 16384];
        match self.socket.read(&mut buffer) {
            Ok(0) => bail!("daemon closed before confirming the device request; no retry was sent"),
            Ok(count) => self.framer.push(&buffer[..count])?,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => {
                return Err(error).context("device request was not confirmed; no retry was sent")
            }
        }
        Ok(())
    }
}

impl Action {
    fn start(
        &self,
        client: &mut Client,
        connection: ConnectionId,
    ) -> Result<(Outbound, Option<ClientOperation>)> {
        if matches!(self, Self::List) {
            return Ok((client.read(connection, Command::DeviceList {})?, None));
        }
        let mut operation = None;
        let outgoing = client.submit(connection, |origin| {
            operation = Some(origin.clone());
            match self {
                Self::Add { device, verifier } => Command::DeviceAdd(DeviceAdd {
                    operation: origin,
                    device: device.clone(),
                    verifier: verifier.clone(),
                }),
                Self::Revoke(device) => Command::DeviceRevoke(DeviceRevoke {
                    operation: origin,
                    device: device.clone(),
                }),
                Self::List => unreachable!("read-only operator list"),
            }
        })?;
        Ok((outgoing, operation))
    }
}
