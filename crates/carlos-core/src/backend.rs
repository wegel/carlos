//! Backend trait abstracting over Codex, Claude, and Grok CLI transports.

use std::ops::Deref;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::Value;

/// Identifies which backend implementation is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
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
pub struct RewindForkRequest {
    pub keep_turns: usize,
    pub drop_turns: usize,
    pub before_turn_id: Option<String>,
    pub before_user_item: Option<RewindUserMessage>,
}

/// Keeps the exact selected native user item separate from its edited draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewindUserMessage {
    pub item_id: String,
    pub text: String,
    pub position: usize,
}

/// Identifies the person or automatic loop that supplied a prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptOrigin {
    Human,
    Automatic,
}

/// Selects when an agent should consume a prompt during an active turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptPriority {
    Steer,
    Queue,
}

/// Keeps the backend informed when the last event consumer disappears.
pub struct EventReceiver<T = String> {
    receiver: mpsc::Receiver<T>,
    on_drop: Option<Box<dyn FnOnce() + Send>>,
}

impl<T> EventReceiver<T> {
    /// Move backend cancellation to the final consumer before forwarding starts.
    pub fn take_close(&mut self) -> Option<Box<dyn FnOnce() + Send>> {
        self.on_drop.take()
    }

    /// Wrap a receiver with a backend-owned cancellation callback.
    pub fn with_close(
        receiver: mpsc::Receiver<T>,
        on_drop: impl FnOnce() + Send + 'static,
    ) -> Self {
        Self {
            receiver,
            on_drop: Some(Box::new(on_drop)),
        }
    }
}

impl<T> From<mpsc::Receiver<T>> for EventReceiver<T> {
    fn from(receiver: mpsc::Receiver<T>) -> Self {
        Self {
            receiver,
            on_drop: None,
        }
    }
}

impl<T> Deref for EventReceiver<T> {
    type Target = mpsc::Receiver<T>;

    fn deref(&self) -> &Self::Target {
        &self.receiver
    }
}

impl<T> Drop for EventReceiver<T> {
    fn drop(&mut self) {
        if let Some(close) = self.on_drop.take() {
            close();
        }
    }
}

pub trait BackendClient {
    fn kind(&self) -> BackendKind;
    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String>;
    /// Send a protocol notice that has no response, when the backend supports it.
    fn notify(&self, _method: &str, _params: Value) -> Result<()> {
        Ok(())
    }
    /// Validate a native form away from the UI and return its result through events.
    fn validate_form(
        &self,
        _id: Value,
        _schema: Value,
        _answer: Option<Value>,
        _revision: u64,
    ) -> Result<()> {
        bail!("Backend does not support local form checks")
    }
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
    /// Correlate local UI work without sending its context to the native server.
    fn request_control_with_context(
        &self,
        method: &str,
        params: Value,
        _context: Value,
    ) -> Result<()> {
        self.request_native_control(method, params)
    }
    /// Ask the backend to publish its current context-window counts.
    fn refresh_context(&self) -> Result<()> {
        Ok(())
    }
    /// Apply lifecycle facts at the caller's handled cursor; None preserves the borrowed line.
    fn handle_event(&self, _line: &str) -> Option<Vec<String>> {
        None
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
    /// Stop only the subprocess Carlos owns after a local fatal error.
    fn abort_connection(&self) {}
    /// Start one asynchronous new connection, leaving all uncertain input unsent.
    fn reconnect(&self, _thread: &str) -> Result<u64> {
        bail!("Backend does not support reconnect")
    }
    /// Claim the single bounded off-stream snapshot at its ordered ready ticket.
    fn take_reconnect_snapshot(
        &self,
        _generation: u64,
    ) -> Option<crate::codex_snapshot::CodexSnapshot> {
        None
    }
    /// Identify this native connection without changing its stable event budget.
    fn connection_generation(&self) -> Option<u64> {
        None
    }
    fn take_events_rx(&mut self) -> Result<EventReceiver>;
    fn stop(&mut self);
}
