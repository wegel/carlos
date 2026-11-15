//! Capture one Codex transport generation without locking the reconnect manager during a reply.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::Duration;

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use super::{CodexClient, Shared};
use crate::backend::{BackendClient, BackendKind, RewindForkRequest};
use crate::backend_commands::{BackendCommands, CommandHandle};
use crate::dispatch_guard::DispatchGuard;
use crate::protocol::AppServerClient;

#[derive(Clone)]
struct Commands {
    permission: DispatchGuard,
    owner: Weak<Shared>,
    connection: Arc<AppServerClient>,
    generation: u64,
}

impl CodexClient {
    pub(super) fn capture(&self) -> Result<CommandHandle> {
        let connection = self.connection()?;
        let commands = Commands {
            permission: DispatchGuard::default(),
            owner: Arc::downgrade(&self.shared),
            connection,
            generation: self.shared.generation.load(Ordering::SeqCst),
        };
        commands.check_current()?;
        Ok(CommandHandle::new(commands))
    }
}

impl Commands {
    fn write_permission(&self, initializing: bool) -> DispatchGuard {
        let captured = self.clone();
        self.permission.clone().and(DispatchGuard::new(move || {
            if initializing {
                captured.check_current()
            } else {
                captured.check()
            }
            .is_ok()
        }))
    }

    fn check(&self) -> Result<()> {
        self.check_current()?;
        let owner = self.owner.upgrade().context("Codex owner has stopped")?;
        ensure!(
            !owner.reconnecting.load(Ordering::SeqCst),
            "captured Codex connection has not finished initializing"
        );
        Ok(())
    }

    fn check_current(&self) -> Result<()> {
        let owner = self.owner.upgrade().context("Codex owner has stopped")?;
        ensure!(
            !owner.stopped.load(Ordering::SeqCst)
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
    fn is_current(&self) -> bool {
        self.check().is_ok()
    }
    fn abort(&self) {
        self.connection.cancel();
    }
    fn guarded(&self, permission: DispatchGuard) -> Result<CommandHandle> {
        let mut commands = self.clone();
        commands.permission = commands.permission.and(permission);
        Ok(CommandHandle::new(commands))
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.check()?;
        self.connection
            .call_guarded(method, params, timeout, self.write_permission(false))
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        timeout: Duration,
    ) -> Result<String> {
        self.check()?;
        crate::protocol_rewind::fork_before_edited_turn(
            &CommandHandle::new(self.clone()),
            thread,
            request,
            timeout,
        )
    }
    fn request_native_control(&self, method: &str, params: Value) -> Result<()> {
        self.request_control_with_context(method, params, Value::Null)
    }
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        context: Value,
    ) -> Result<()> {
        self.check()?;
        self.connection.request_control_guarded(
            method,
            params,
            context,
            self.write_permission(false),
        )
    }
    fn refresh_context(&self) -> Result<()> {
        self.check()?;
        self.connection.refresh_context()
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.check_current()?;
        self.connection
            .respond_guarded(id, result, self.write_permission(true))
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.check_current()?;
        self.connection
            .respond_error_guarded(id, code, message, self.write_permission(true))
    }
}
