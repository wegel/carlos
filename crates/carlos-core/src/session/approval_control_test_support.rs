//! Record exact guarded callback writes while forbidding ordinary chat requests in core tests.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::backend::{BackendClient, BackendKind, EventReceiver};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;

pub(super) type Calls = Arc<Mutex<Vec<(Value, Value)>>>;

pub(super) struct Peer {
    pub(super) commands: CommandHandle,
    pub(super) calls: Calls,
    pub(super) fail: Arc<AtomicBool>,
}

#[derive(Clone)]
struct Writer {
    kind: BackendKind,
    calls: Calls,
    fail: Arc<AtomicBool>,
    permission: DispatchGuard,
}

impl Peer {
    pub(super) fn new(kind: BackendKind) -> Self {
        let calls = Calls::default();
        let fail = Arc::new(AtomicBool::new(false));
        let commands = CommandHandle::new(Writer {
            kind,
            calls: calls.clone(),
            fail: fail.clone(),
            permission: Default::default(),
        });
        Self {
            commands,
            calls,
            fail,
        }
    }
}

impl BackendCommands for Writer {
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        Ok(CommandHandle::new(Self {
            permission,
            ..self.clone()
        }))
    }
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn call(&self, method: &str, _: Value, _: Duration) -> Result<String> {
        panic!("No synthetic request is allowed: {method}")
    }
    fn respond(&self, id: &Value, body: Value) -> Result<()> {
        self.permission.check()?;
        self.calls
            .lock()
            .expect("native answers")
            .push((id.clone(), body));
        if self.fail.load(Ordering::SeqCst) {
            bail!("fixed answer failure")
        }
        Ok(())
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no native error response expected")
    }
}

impl BackendClient for Peer {
    fn kind(&self) -> BackendKind {
        self.commands.kind()
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.commands.call(method, params, timeout)
    }
    fn respond(&self, id: &Value, body: Value) -> Result<()> {
        self.commands.respond(id, body)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.commands.respond_error(id, code, message)
    }
    fn validate_form(&self, _: Value, _: Value, _: Option<Value>, _: u64) -> Result<()> {
        Ok(())
    }
    fn capture_commands(&self) -> Result<CommandHandle> {
        Ok(self.commands.clone())
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("unused")
    }
    fn stop(&mut self) {}
}
