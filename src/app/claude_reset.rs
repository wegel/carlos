//! Clears one native transcript while preserving process tasks, unanswered callbacks, and accepted inputs.

use serde_json::Value;

use crate::claude_backend::ClaudeTranslationState;

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
    app.claude
        .reset_prompts(message["user_message_uuid"].as_str());
    let approval = app.approval.pending.take();
    let questions = std::mem::take(&mut app.claude.questions);
    let old_id = app.thread_id.clone();
    app.reset_for_forked_thread(old_id);
    app.approval.pending = approval;
    app.claude.questions = questions;
    app.thread_name = None;
    app.claude.history = Default::default();
    app.claude.translation = ClaudeTranslationState::default();
    super::claude_context::invalidate(app);
    app.claude.activity.compact = None;
    app.claude.activity.thinking = None;
    app.claude.activity.transient = None;
    app.claude.phase = ClaudePhase::Starting;
    app.ralph_runtime.stop_after_failure();
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
        app.claude.phase = ClaudePhase::Ready;
    }
}
