//! Apply the existing helper switch, native reset, and init fences.

use serde_json::Value;

use super::history::ClaudeHistory;
use super::state::{ClaudePhase, ClaudeState};
use crate::session::state::SessionState;

impl SessionState {
    /// Mark startup before the helper can receive a prompt.
    pub(crate) fn start_claude(&mut self) {
        self.claude.phase = ClaudePhase::Starting;
    }

    /// Release a failed edit without reopening a closed helper.
    pub(crate) fn ready_claude_after_edit_failure(&mut self) {
        if self.claude.phase != ClaudePhase::Closed {
            self.claude.phase = ClaudePhase::Ready;
        }
    }

    /// Close the current query at the existing terminal failure point.
    pub(crate) fn close_claude(&mut self) -> bool {
        self.claude.phase = ClaudePhase::Closed;
        let editing = self.claude_edit_progress().is_some();
        if editing {
            self.end_claude_edit();
        }
        editing
    }

    /// Fail accepted work after the app retires approval editors, preserving the old order.
    pub(crate) fn fail_claude_session_work(&mut self) {
        self.fail_claude_prompts();
        self.fail_claude_process();
        self.ralph_stop_after_failure();
    }

    /// Reset all helper facts once per switch, retaining only native-choice inheritance.
    pub(crate) fn reset_claude_for_switch(&mut self, generation: u64) {
        self.claude = ClaudeState {
            inherits_model: self.claude.inherits_model,
            inherits_effort: self.claude.inherits_effort,
            enabled: true,
            generation,
            phase: ClaudePhase::Starting,
            ..ClaudeState::default()
        };
    }

    /// Remember a new reset key before the caller retires transcript and view state.
    pub(crate) fn begin_claude_reset(&mut self, message: &Value) -> bool {
        let key = message["new_conversation_id"].as_str().unwrap_or("");
        if self.claude.reset_key.as_deref() == Some(key) {
            return false;
        }
        self.claude.reset_key = Some(key.to_string());
        self.claude.awaiting_reset_init = true;
        self.reset_claude_prompts(message["user_message_uuid"].as_str());
        true
    }

    /// Clear conversation sources and edit progress while preserving process facts.
    pub(crate) fn clear_claude_history(&mut self) {
        self.claude.history = ClaudeHistory::default();
        self.clear_claude_edit();
    }

    /// Finish a native reset after the app has retired its local history editors.
    pub(crate) fn finish_claude_reset(&mut self) {
        self.clear_claude_conversation_activity();
        self.start_claude();
        self.ralph_stop_after_failure();
        self.set_status("Claude cleared the conversation; waiting for its native session identity");
    }

    /// Read only an accepted reset's string native identity, without adding validation.
    pub(crate) fn claude_reset_identity<'a>(&self, message: &'a Value) -> Option<&'a str> {
        if !self.claude.awaiting_reset_init {
            return None;
        }
        message["session_id"].as_str()
    }

    /// Mark the accepted native init ready after the caller adopts its thread identity.
    pub(crate) fn finish_claude_reset_init(&mut self) {
        self.claude.awaiting_reset_init = false;
        self.claude.phase = ClaudePhase::Ready;
    }
}
