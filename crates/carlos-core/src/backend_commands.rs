//! Share native commands without transferring a backend's event receiver or normalizer.

use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

use crate::backend::{
    BackendClient, BackendKind, EventReceiver, PromptOrigin, PromptPriority, RewindForkRequest,
};

/// Owner-held lifecycle intent; workers never inspect or change these native prompt ids.
#[derive(Default)]
pub struct InterruptIntent {
    pub(crate) generation: u64,
    pub(crate) epoch: u64,
    pub(crate) prompts: Vec<String>,
}

/// A captured native connection that workers can use while its owner handles events.
pub trait BackendCommands: Send + Sync {
    /// Identify the native adapter behind this captured connection.
    fn kind(&self) -> BackendKind;
    /// Run a native request without holding an owner or connection-manager lock.
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
    /// Submit text with its native author and queue policy.
    fn submit_prompt(
        &self,
        method: &str,
        params: Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        let _ = (origin, priority);
        self.call(method, params, timeout)
    }
    /// Fork from the captured native prompt rather than a replacement conversation.
    fn fork_from_rewind(
        &self,
        _thread: &str,
        _request: RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        bail!("backend does not support rewind fork")
    }
    /// Answer one native callback on this connection.
    fn respond(&self, id: &Value, result: Value) -> Result<()>;
    /// Reject one native callback on this connection.
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()>;
}

/// Clone a fixed command endpoint, never a receiver or a mutable helper selector.
#[derive(Clone)]
pub struct CommandHandle(Arc<dyn BackendCommands>);

impl CommandHandle {
    /// Capture a concrete command adapter for worker use.
    pub fn new(commands: impl BackendCommands + 'static) -> Self {
        Self(Arc::new(commands))
    }
}

impl Deref for CommandHandle {
    type Target = dyn BackendCommands;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

// Temporary embedded-terminal bridge; EP019b removes BackendClient command consumers.
impl BackendClient for CommandHandle {
    fn kind(&self) -> BackendKind {
        self.0.kind()
    }
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        self.0.call(method, params, timeout)
    }
    fn submit_prompt(
        &self,
        method: &str,
        params: Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        self.0
            .submit_prompt(method, params, origin, priority, timeout)
    }
    fn fork_from_rewind(
        &self,
        thread: &str,
        request: RewindForkRequest,
        timeout: Duration,
    ) -> Result<String> {
        self.0.fork_from_rewind(thread, request, timeout)
    }
    fn respond(&self, id: &Value, result: Value) -> Result<()> {
        self.0.respond(id, result)
    }
    fn respond_error(&self, id: &Value, code: i64, message: &str) -> Result<()> {
        self.0.respond_error(id, code, message)
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver> {
        bail!("command handles do not own event receivers")
    }
    fn stop(&mut self) {}
}
