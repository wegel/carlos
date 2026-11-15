//! Record guarded native fork/send writes without rebuilding the removed backend-client mock.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::backend::{BackendKind, RewindForkRequest};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;

/// Keep the exact native requests observed by one captured writer.
#[derive(Default)]
pub(super) struct Writes {
    pub(super) forks: Vec<(String, RewindForkRequest)>,
    pub(super) starts: Vec<Value>,
}

#[derive(Clone)]
struct Writer {
    kind: BackendKind,
    writes: Arc<Mutex<Writes>>,
    permission: DispatchGuard,
}

/// Capture the command interface used by the ordinary worker and its exact request trace.
pub(super) fn rewind_peer() -> (CommandHandle, Arc<Mutex<Writes>>) {
    peer(BackendKind::Codex)
}

/// Capture the fixed guarded write seam with Grok's native fork response.
pub(super) fn grok_peer() -> (CommandHandle, Arc<Mutex<Writes>>) {
    peer(BackendKind::Grok)
}

fn peer(kind: BackendKind) -> (CommandHandle, Arc<Mutex<Writes>>) {
    let writes = Arc::new(Mutex::new(Writes::default()));
    let commands = CommandHandle::new(Writer {
        kind,
        writes: writes.clone(),
        permission: Default::default(),
    });
    (commands, writes)
}

impl BackendCommands for Writer {
    fn kind(&self) -> BackendKind {
        self.kind
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        Ok(CommandHandle::new(Self {
            permission: self.permission.clone().and(permission),
            ..self.clone()
        }))
    }
    fn call(&self, method: &str, params: Value, _: Duration) -> Result<String> {
        self.permission.check()?;
        assert_eq!(method, "turn/start");
        self.writes.lock().expect("writes").starts.push(params);
        Ok(json!({"jsonrpc":"2.0","result":{}}).to_string())
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        _: Duration,
    ) -> Result<String> {
        self.permission.check()?;
        self.writes
            .lock()
            .expect("writes")
            .forks
            .push((thread.into(), request));
        if self.kind == BackendKind::Grok {
            return Ok(json!({"jsonrpc":"2.0","result":{"thread":{"id":"grok-fork"}}}).to_string());
        }
        Ok(
            json!({"jsonrpc":"2.0","result":{"thread":{"id":"thread-fork","turns":[{"items":[
                {"type":"userMessage","content":[{"type":"text","text":"first"}]},
                {"type":"agentMessage","text":"reply one"}
            ]}]}}})
            .to_string(),
        )
    }
    fn respond(&self, _: &Value, _: Value) -> Result<()> {
        bail!("no callback expected")
    }
    fn respond_error(&self, _: &Value, _: i64, _: &str) -> Result<()> {
        bail!("no callback expected")
    }
}
