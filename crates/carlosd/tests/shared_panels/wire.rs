//! Supply only Unix I/O and fixed fixture deadlines around the production pure client's effects.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use carlos_client::codec::{Framer, Incoming};
use carlos_client::{Client, ConnectionId, Effects, Outbound};
use carlos_proto::commands::RequestId;
use serde_json::Value;

pub struct Wire {
    socket: UnixStream,
    frames: Framer,
    pub connection: ConnectionId,
    pub methods: Vec<String>,
}

impl Wire {
    pub fn connect(path: &Path, client: &mut Client) -> Self {
        let socket = UnixStream::connect(path).expect("actual fixture daemon socket");
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .expect("fixture read deadline");
        socket
            .set_write_timeout(Some(Duration::from_secs(3)))
            .expect("fixture write deadline");
        let hello = client
            .begin("shared-panel-consumer".into())
            .expect("private hello");
        let mut wire = Self {
            socket,
            frames: Framer::default(),
            connection: hello.connection(),
            methods: Vec::new(),
        };
        wire.send(&hello);
        wire.until(client, |client| client.is_ready(hello.connection()));
        wire
    }

    pub fn send_effects(&mut self, effects: Effects) {
        assert!(
            effects.failures.is_empty(),
            "fixture client failure: {:?}",
            effects.failures.first().map(|failure| &failure.error)
        );
        assert!(
            effects.replies.is_empty(),
            "shared readers consume their own replies"
        );
        for outgoing in effects.outgoing {
            self.send(&outgoing);
        }
    }

    pub fn send(&mut self, outgoing: &Outbound) {
        let value: Value =
            serde_json::from_slice(outgoing.bytes()).expect("production request codec");
        self.methods
            .push(value["method"].as_str().expect("typed method").to_owned());
        self.socket
            .write_all(outgoing.bytes())
            .expect("write production client frame");
    }

    pub fn until(&mut self, client: &mut Client, done: impl Fn(&Client) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !done(client) {
            assert!(
                Instant::now() < deadline,
                "fixture reaches checked panel state"
            );
            let message = self.next();
            let effects = client
                .receive(self.connection, message)
                .expect("checked real daemon message");
            self.send_effects(effects);
        }
    }

    pub fn hold_reply(&mut self, client: &mut Client, id: RequestId) -> Incoming {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "original fixture reply arrives");
            let incoming = self.next();
            if matches!(&incoming, Incoming::Success { id:received, .. } if received==&id) {
                return incoming;
            }
            let effects = client
                .receive(self.connection, incoming)
                .expect("other checked message");
            self.send_effects(effects);
        }
    }

    fn next(&mut self) -> Incoming {
        loop {
            if let Some(incoming) = self.frames.next_message().expect("production reply codec") {
                return incoming;
            }
            let mut bytes = [0; 65536];
            let count = self
                .socket
                .read(&mut bytes)
                .expect("fixture daemon replies without inference");
            assert!(count > 0, "fixture socket remains live");
            self.frames
                .push(&bytes[..count])
                .expect("production bounded frame");
        }
    }
}
