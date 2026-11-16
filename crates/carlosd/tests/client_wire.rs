//! Drive the production Unix listener with the pure client's codec and private reconnect state.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use carlos_client::codec::{self, Framer, Incoming};
use carlos_client::{Client, Connection, Effects, Outbound, Welcome};
use carlos_proto::commands::{Command, HelloResult, RequestId, Sessions};
use carlosd::server::{self, Config};
use tempfile::TempDir;

struct Daemon {
    _root: TempDir,
    path: PathBuf,
    stop: Arc<AtomicBool>,
    owner: Option<JoinHandle<()>>,
}

impl Daemon {
    fn new() -> Self {
        let root = tempfile::Builder::new()
            .prefix("carlos-client-wire-")
            .tempdir_in("/tmp")
            .expect("private fixture");
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700))
            .expect("private fixture permissions");
        let path = root.path().join("daemon.sock");
        let config = Config {
            socket: path.clone(),
            state: root.path().join("state"),
            codex_socket: None,
            listen: None,
            allowed_origin: None,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let owner = thread::spawn(move || server::run(config, flag).expect("daemon"));
        Self {
            _root: root,
            path,
            stop,
            owner: Some(owner),
        }
    }

    fn connect(&self) -> Wire {
        let deadline = Instant::now() + Duration::from_secs(3);
        let socket = loop {
            match UnixStream::connect(&self.path) {
                Ok(socket) => break socket,
                Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                Err(error) => panic!("daemon listener: {error}"),
            }
        };
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("read deadline");
        socket
            .set_write_timeout(Some(Duration::from_secs(3)))
            .expect("write deadline");
        Wire {
            socket,
            frames: Framer::default(),
            serial: 0,
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let result = self.owner.take().expect("owner").join();
        if !thread::panicking() {
            result.expect("daemon joined");
        }
    }
}

struct Wire {
    socket: UnixStream,
    frames: Framer,
    serial: i64,
}

impl Wire {
    fn call(&mut self, command: Command) -> Incoming {
        self.serial += 1;
        let request = codec::request(RequestId::Integer(self.serial), command);
        self.socket
            .write_all(&codec::encode(&request).expect("encode"))
            .expect("write");
        loop {
            if let Some(message) = self.frames.next_message().expect("decode") {
                match &message {
                    Incoming::Success { id, .. } | Incoming::Failure { id: Some(id), .. } => {
                        assert_eq!(id, &request.id);
                        return message;
                    }
                    _ => panic!("unexpected notification before any interest"),
                }
            }
            let mut buffer = [0; 4096];
            let read = self.socket.read(&mut buffer).expect("read");
            assert!(read > 0, "daemon closed stream");
            self.frames.push(&buffer[..read]).expect("bounded frame");
        }
    }

    fn drive(&mut self, client: &mut Client, outgoing: &Outbound) -> Effects {
        self.socket
            .write_all(outgoing.bytes())
            .expect("write pure client request");
        loop {
            let message = self.next_message();
            let complete = match &message {
                Incoming::Success { id, .. } | Incoming::Failure { id: Some(id), .. } => {
                    *id == outgoing.id()
                }
                _ => false,
            };
            let effects = client
                .receive(outgoing.connection(), message)
                .expect("checked client reply");
            if complete {
                return effects;
            }
        }
    }

    fn next_message(&mut self) -> Incoming {
        loop {
            if let Some(message) = self.frames.next_message().expect("decode") {
                return message;
            }
            let mut bytes = [0; 4096];
            let count = self.socket.read(&mut bytes).expect("read peer");
            assert!(count > 0, "peer remains connected");
            self.frames
                .push(&bytes[..count])
                .expect("bounded stream frame");
        }
    }

    fn hello(&mut self, command: Command) -> HelloResult {
        let Incoming::Success { result, .. } = self.call(command) else {
            panic!("greeting accepted");
        };
        serde_json::from_value(result).expect("typed greeting")
    }
}

#[test]
fn actual_daemon_agrees_with_codec_credentials_and_never_reissues_an_unknown_operation() {
    let daemon = Daemon::new();
    let mut connection = Connection::default();
    let (first, hello) = connection.begin("wire-test".into()).expect("begin");
    let mut wire = daemon.connect();
    assert_eq!(
        connection.accept(first, wire.hello(hello)).expect("greet"),
        Welcome::Fresh
    );
    let operation = connection
        .next_operation(first)
        .expect("locally issued, never written");
    drop(wire);
    connection.disconnect(first);
    let (second, hello) = connection.begin("wire-test".into()).expect("reconnect");
    let mut wire = daemon.connect();
    assert_eq!(
        connection
            .accept(second, wire.hello(hello))
            .expect("resume private rights"),
        Welcome::Resumed
    );
    assert_eq!(connection.next_operation(second).expect("next").id.get(), 2);
    let Incoming::Failure { error, .. } = wire.call(Command::Operation { operation }) else {
        panic!("unknown outcome");
    };
    assert_eq!(error.code, -32006);
    let Incoming::Success { result, .. } = wire.call(Command::Watch {}) else {
        panic!("list still works");
    };
    assert!(serde_json::from_value::<Sessions>(result)
        .expect("typed list")
        .sessions
        .is_empty());
}

#[test]
fn coherent_client_queries_its_unwritten_operation_after_private_resume_without_starting_a_backend()
{
    let daemon = Daemon::new();
    let mut client = Client::default();
    let mut wire = daemon.connect();
    let greeting = client.begin("coherent-client".into()).expect("begin");
    assert_eq!(
        wire.drive(&mut client, &greeting).welcome,
        Some(Welcome::Fresh)
    );
    let sent = client
        .submit(greeting.connection(), |operation| Command::Send {
            session: "a".repeat(32).parse().expect("unopened session"),
            operation,
            text: "unwritten local text".into(),
            policy: Default::default(),
        })
        .expect("retain before platform write");
    let request: carlos_proto::commands::Request =
        serde_json::from_slice(sent.bytes()).expect("typed request");
    let Command::Send { operation, .. } = request.command else {
        panic!("send");
    };
    drop(wire);
    client.disconnect(greeting.connection());
    let mut wire = daemon.connect();
    let greeting = client.begin("coherent-client".into()).expect("reconnect");
    let mut effects = wire.drive(&mut client, &greeting);
    assert_eq!(effects.welcome, Some(Welcome::Resumed));
    assert_eq!(
        effects.outgoing.len(),
        1,
        "only the outcome read is automatic"
    );
    let recover = effects.outgoing.pop().expect("private outcome query");
    let request: carlos_proto::commands::Request =
        serde_json::from_slice(recover.bytes()).expect("query");
    assert!(matches!(request.command, Command::Operation { operation: read } if read == operation));
    let effects = wire.drive(&mut client, &recover);
    assert_eq!(effects.failures[0].error.code, -32006);
    assert!(effects.outgoing.is_empty());
    assert_private_recovery(&client, &operation);
    let watch = client
        .read(greeting.connection(), Command::Watch {})
        .expect("catalogue read");
    assert!(wire.drive(&mut client, &watch).catalog);
    assert_eq!(
        client.catalog().count(),
        0,
        "no backend session was created or resumed"
    );
}

fn assert_private_recovery(
    client: &Client,
    operation: &carlos_proto::submissions::ClientOperation,
) {
    let record = client.operations().get(operation).expect("private result");
    assert!(matches!(
        record.state,
        carlos_proto::submissions::OperationState::Uncertain { .. }
    ));
    assert_eq!(record.text(), Some("unwritten local text"));
}
