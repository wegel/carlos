//! Record native diagnostics before the app recovers local drafts and callback editors.

use serde_json::Value;

use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::text::safe_label;

/// Describe whether the caller must recover drafts after a backend failure.
pub(crate) struct ClaudeEventUpdate {
    pub fail_session: bool,
    pub closed: bool,
}

impl SessionState {
    /// Record stderr or error rows and decide whether the query must fail.
    pub(crate) fn receive_claude_event(
        &mut self,
        method: &str,
        frame: &Value,
    ) -> ClaudeEventUpdate {
        let mut result = ClaudeEventUpdate {
            fail_session: false,
            closed: false,
        };
        match method {
            "claude/stderr" => {
                if let Some(text) = frame["text"].as_str() {
                    self.record_claude_notice(format!("stderr:{text}"), text);
                }
            }
            "claude/error" => {
                if frame["error"]["code"] == "unsupported_dialog" {
                    self.append_message(
                        Role::System,
                        format!(
                            "{} (request {}, kind {})",
                            frame["error"]["message"]
                                .as_str()
                                .unwrap_or("Unsupported Claude dialog"),
                            safe_label(frame["callbackId"].as_str().unwrap_or("unknown")),
                            safe_label(frame["dialogKind"].as_str().unwrap_or("unknown")),
                        ),
                    );
                } else {
                    self.record_claude_error(&frame["error"]);
                    result.fail_session = true;
                }
            }
            "claude/closed" => {
                result.fail_session = true;
                result.closed = true;
            }
            _ => {}
        }
        result
    }

    /// Retain the diagnostic before helper cleanup, without escaping its existing prose.
    pub(crate) fn record_claude_error(&mut self, error: &Value) {
        let message = error["message"].as_str().unwrap_or("Claude SDK error");
        self.append_message(Role::System, message);
        self.set_status(message);
    }

    /// Record closure only after the app finishes its existing failure recovery.
    pub(crate) fn record_claude_closed(&mut self, frame: &Value) {
        let reason = frame["reason"].as_str().unwrap_or("unknown");
        self.set_status(format!("Claude closed: {reason}"));
        self.append_message(Role::System, format!("Claude closed: {reason}"));
    }
}
