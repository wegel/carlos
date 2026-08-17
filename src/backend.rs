//! Backend trait abstracting over Codex, Claude, and Grok CLI transports.

use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

/// Identifies which backend implementation is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BackendKind {
    Codex,
    Claude,
    Grok,
}

impl BackendKind {
    /// Whether the UI should append the user prompt locally after `turn/start`.
    pub(crate) fn echoes_local_user_prompt(self) -> bool {
        matches!(self, Self::Claude | Self::Grok)
    }

    /// Whether a later prompt must wait behind an already-queued follow-up.
    pub(crate) fn queues_follow_up_turns(self) -> bool {
        matches!(self, Self::Claude | Self::Grok)
    }

    /// Whether input during an active turn should queue instead of `turn/steer`.
    pub(crate) fn queues_instead_of_steer(self) -> bool {
        matches!(self, Self::Grok)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RewindForkRequest {
    pub(crate) keep_turns: usize,
    pub(crate) drop_turns: usize,
}

pub(crate) trait BackendClient {
    fn kind(&self) -> BackendKind;
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
    fn fork_from_rewind(
        &self,
        _thread_id: &str,
        _request: RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        bail!("backend does not support rewind fork")
    }
    fn respond(&self, request_id: &Value, result: Value) -> Result<()>;
    fn respond_error(&self, request_id: &Value, code: i64, message: &str) -> Result<()>;
    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>>;
    fn stop(&mut self);
}
