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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RewindForkRequest {
    pub(crate) keep_turns: usize,
    pub(crate) drop_turns: usize,
    pub(crate) before_turn_id: Option<String>,
}

/// Identifies the person or automatic loop that supplied a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptOrigin {
    Human,
    Automatic,
}

/// Selects when an agent should consume a prompt during an active turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptPriority {
    Steer,
    Queue,
}

pub(crate) trait BackendClient {
    fn kind(&self) -> BackendKind;
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
    /// Submit a prompt without adding backend-specific fields to other backends' JSON.
    fn submit_prompt(
        &self,
        method: &str,
        params: Value,
        _origin: PromptOrigin,
        _priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        self.call(method, params, timeout)
    }
    /// Apply choices immediately when the backend supports live settings.
    fn apply_runtime_settings(&self, _model: Option<&str>, _effort: Option<&str>) -> Result<bool> {
        Ok(false)
    }
    /// Send an optional native control whose reply arrives through backend events.
    fn request_native_control(&self, _method: &str, _params: Value) -> Result<()> {
        bail!("backend does not support native controls")
    }
    /// Ask the backend to publish its current context-window counts.
    fn refresh_context(&self) -> Result<()> {
        Ok(())
    }
    /// Release a backend's event budget after the UI has processed a frame.
    fn acknowledge_event(&self, _length: usize) {}
    /// Start a quota read whose response arrives on the event stream, if supported.
    fn request_rate_limits(&self) -> Result<Option<String>> {
        Ok(None)
    }
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
