//! Clears one native transcript while preserving process tasks, unanswered callbacks, and accepted inputs.

use serde_json::Value;

use super::claude_state::ClaudePhase;
use super::state::AppState;

/// Apply the reset once; the next observed native init supplies the actual session ID.
pub(super) fn receive(app: &mut AppState, message: &Value) {
    let key = message["new_conversation_id"].as_str().unwrap_or("");
    if app.claude.reset_key.as_deref() == Some(key) {
        return;
    }
    app.claude.reset_key = Some(key.to_string());
    app.claude.awaiting_reset_init = true;
    super::claude_prompts::reset(app, message["user_message_uuid"].as_str());
    let old_id = app.session.thread_id().clone();
    app.reset_preserving_approvals(old_id);
    app.session.set_thread_name(None);
    app.claude.history = Default::default();
    super::claude_context::invalidate(app);
    app.claude.activity.compact = None;
    app.claude.activity.thinking = None;
    app.claude.activity.transient = None;
    app.session.set_claude_phase(ClaudePhase::Starting);
    app.session.ralph_stop_after_failure();
    app.set_status("Claude cleared the conversation; waiting for its native session identity");
}

/// Adopt only an init that the transport's one-shot reset guard already accepted.
pub(super) fn initialize(app: &mut AppState, message: &Value) {
    if !app.claude.awaiting_reset_init {
        return;
    }
    if let Some(id) = message["session_id"].as_str() {
        app.set_thread_id(id);
        app.claude.awaiting_reset_init = false;
        app.session.set_claude_phase(ClaudePhase::Ready);
    }
}
