//! Apply already-normalized native process facts without translating lifecycle events.

use serde_json::Value;

use crate::session::state::SessionState;

impl SessionState {
    /// Record main-process state and refresh counts only for the exact running event.
    pub fn record_claude_activity(&mut self, message: &Value) {
        self.claude.activity.state(message);
        if message["state"] == "running" {
            self.refresh_claude_context();
        }
    }

    /// Replace the optional native thinking estimate.
    pub(crate) fn record_claude_thinking(&mut self, message: &Value) {
        self.claude.activity.thinking = message["estimated_tokens"].as_u64();
    }

    /// Replace the temporary label from an ordinary native status fact.
    pub fn record_claude_status(&mut self, message: &Value) {
        self.claude.activity.transient = message["label"].as_str().map(str::to_string);
    }

    /// Clear temporary work after the app appends any native result error.
    pub(crate) fn finish_claude_activity(&mut self) {
        self.claude.activity.transient = None;
        self.claude.activity.thinking = None;
    }

    /// Fence old counts and clear conversation activity while retaining process work.
    pub(crate) fn clear_claude_conversation_activity(&mut self) {
        self.invalidate_claude_context();
        self.claude.activity.compact = None;
        self.claude.activity.thinking = None;
        self.claude.activity.transient = None;
    }

    /// Forget main activity and live membership after existing failure recovery.
    pub(crate) fn fail_claude_process(&mut self) {
        self.claude.activity = Default::default();
        self.claude.tasks.live.clear();
    }
}
