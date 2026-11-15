//! Observe the real owner reducer around a deterministic controller pipe without changing its policy.

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use carlos_proto::session::Snapshot;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};
use serde_json::Value;

use super::commands::Commands;
use super::event_test_support::{command_client, SESSION};
use super::ClaudeClient;
use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::backend_commands::{CommandHandle, InterruptIntent};
use crate::engine::{EngineCommand, EngineHandle, EngineReply};
use crate::session::state::SessionState;

/// Bound every test wait without fixing native event order through sleeps.
pub(super) const TIMEOUT: Duration = Duration::from_secs(5);
/// Confirm that the owner consumed the scripted result and idle facts.
pub(super) const MARKER: &str = "owner has applied result and idle";

/// Keep an actual owner, captured helper commands and normalized fact trace together.
pub(super) struct Fixture {
    pub(super) engine: EngineHandle,
    pub(super) commands: Commands,
    facts: Arc<Mutex<Vec<Value>>>,
}

struct Observed {
    client: ClaudeClient,
    facts: Arc<Mutex<Vec<Value>>>,
}

impl Fixture {
    /// Start the real helper pipes and owner around one deterministic peer.
    pub(super) fn start(script: &str, native_status: &str, fatal: bool) -> Self {
        let mut command = Command::new("python3");
        command.args([
            "-u",
            "-c",
            script,
            native_status,
            if fatal { "yes" } else { "no" },
        ]);
        let (client, receiver) = command_client(command, 1024);
        let commands = client.commands().expect("fixed helper capture");
        let facts = Arc::new(Mutex::new(Vec::new()));
        let observed = Observed {
            client,
            facts: facts.clone(),
        };
        let mut session = SessionState::new(SESSION.into(), Default::default(), false);
        session.enable_claude();
        session.set_client_identity(carlos_proto::session::Backend::Claude, "test".into());
        let (engine, _) = EngineHandle::spawn(session, Box::new(observed), receiver);
        Self {
            engine,
            commands,
            facts,
        }
    }

    /// Admit one command and require the owner to answer within the test deadline.
    pub(super) fn request(&self, command: EngineCommand) -> EngineReply {
        self.engine
            .request(command)
            .expect("bounded admission")
            .recv_timeout(TIMEOUT)
            .expect("responsive owner")
    }

    /// Read the current accepted owner snapshot without polling a native process.
    pub(super) fn snapshot(&self) -> Snapshot {
        let EngineReply::Snapshot(snapshot) = self.request(EngineCommand::Snapshot { limit: 100 })
        else {
            panic!("snapshot")
        };
        *snapshot
    }

    /// Wait for a factual owner state while leaving its native work free to run.
    pub(super) fn until(&self, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let snapshot = self.snapshot();
            if predicate(&snapshot) {
                return snapshot;
            }
            assert!(Instant::now() < deadline, "owner reaches native step");
            std::thread::yield_now();
        }
    }

    /// Observe only the original operation's final private result.
    pub(super) fn outcome(&self, id: u64) -> OperationResult {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            let EngineReply::Outcomes(results) = self.request(EngineCommand::Outcomes) else {
                panic!("private outcomes")
            };
            if let Some(result) = results.into_iter().find(|result| {
                result.operation == operation(id)
                    && !matches!(
                        result.state,
                        OperationState::Pending | OperationState::Queued
                    )
            }) {
                return result;
            }
            assert!(Instant::now() < deadline, "original control settles");
            std::thread::yield_now();
        }
    }

    /// Copy the normalized facts returned by the real Claude receiver.
    pub(super) fn facts(&self) -> Vec<Value> {
        self.facts.lock().expect("observed facts").clone()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        self.engine.shutdown();
        self.engine.join().expect("owner and bounded workers stop");
    }
}

/// Allocate a distinct operation under the same deterministic device identity.
pub(super) fn operation(id: u64) -> ClientOperation {
    ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: id.to_string().parse().expect("counter"),
    }
}

impl Observed {
    fn record(&self, output: &[String]) {
        self.facts.lock().expect("observed facts").extend(
            output
                .iter()
                .map(|line| serde_json::from_str(line).expect("normalized fact")),
        );
    }
}

impl BackendClient for Observed {
    fn kind(&self) -> BackendKind {
        self.client.kind()
    }
    fn connection_generation(&self) -> Option<u64> {
        self.client.connection_generation()
    }
    fn capture_commands(&self) -> Result<CommandHandle> {
        self.client.capture_commands()
    }
    fn prepare_interrupt(&self) -> InterruptIntent {
        self.client.prepare_interrupt()
    }
    fn finish_interrupt(&self, intent: InterruptIntent, accepted: bool) {
        self.client.finish_interrupt(intent, accepted);
    }
    fn defers_terminal_events(&self) -> bool {
        self.client.defers_terminal_events()
    }
    fn take_control_events(&self) -> Vec<String> {
        let output = self.client.take_control_events();
        self.record(&output);
        output
    }
    fn handle_event(&self, line: &str) -> Option<Vec<String>> {
        let output = self.client.handle_event(line)?;
        self.record(&output);
        Some(output)
    }
    fn acknowledge_event(&self, bytes: usize) {
        self.client.acknowledge_event(bytes);
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.client.call(method, params, timeout)
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.client.respond(id, result)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.client.respond_error(id, code, message)
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("receiver already transferred")
    }
    fn stop(&mut self) {
        self.client.stop();
    }
}
