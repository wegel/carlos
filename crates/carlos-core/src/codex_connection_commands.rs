//! Capture one Codex transport generation without locking the reconnect manager during a reply.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use super::{CodexClient, Shared};
use crate::backend::{BackendClient, BackendKind, RewindForkRequest};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::protocol::AppServerClient;

struct Commands {
    owner: Weak<Shared>,
    connection: Arc<AppServerClient>,
    generation: u64,
}

impl CodexClient {
    pub(super) fn capture(&self) -> Result<CommandHandle> {
        let connection = self.connection()?;
        let commands = Commands {
            owner: Arc::downgrade(&self.shared),
            connection,
            generation: self.shared.generation.load(Ordering::SeqCst),
        };
        commands.check()?;
        Ok(CommandHandle::new(commands))
    }
}

impl Commands {
    fn check(&self) -> Result<()> {
        let owner = self.owner.upgrade().context("Codex owner has stopped")?;
        ensure!(
            !owner.stopped.load(Ordering::SeqCst)
                && !owner.reconnecting.load(Ordering::SeqCst)
                && owner.generation.load(Ordering::SeqCst) == self.generation,
            "captured Codex connection has retired"
        );
        let current = owner.current.lock().expect("Codex connection poisoned");
        ensure!(
            current
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &self.connection)),
            "captured Codex connection was replaced"
        );
        Ok(())
    }
}

impl BackendCommands for Commands {
    fn kind(&self) -> BackendKind {
        BackendKind::Codex
    }
    fn connection_generation(&self) -> Option<u64> {
        Some(self.generation)
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.check()?;
        self.connection.call(method, params, timeout)
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        timeout: Duration,
    ) -> Result<String> {
        self.check()?;
        self.connection.fork_from_rewind(thread, request, timeout)
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.check()?;
        self.connection.respond(id, result)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.check()?;
        self.connection.respond_error(id, code, message)
    }
}
