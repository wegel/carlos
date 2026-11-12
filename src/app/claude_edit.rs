//! Bridge submitted Claude branch requests to local recovery and composer effects.

use serde_json::Value;

use super::claude_state::ClaudePhase;
use super::models::Role;
use super::state::AppState;
use crate::backend::PromptOrigin;

/// Keep the submitted edit until the fork and replacement startup are both ready.
pub(super) fn begin(app: &mut AppState, text: String) {
    app.session.begin_claude_edit(text);
    app.view.claude_edit.failed = false;
    retire_queue(app);
    app.view.viewport.auto_follow_bottom = true;
}

/// Report an accepted request independently of a local failure badge.
pub(super) fn waiting(app: &AppState) -> bool {
    app.session.claude_edit_progress().is_some()
}

/// Restore failed text without reopening a closed helper or replacing a newer draft.
pub(super) fn failed(app: &mut AppState, message: &str) {
    super::claude_session_history::restore_edit(app);
    if app.session.claude().phase() != ClaudePhase::Closed {
        app.session.set_claude_phase(ClaudePhase::Ready);
    }
    unavailable(app, message);
}

/// Explain an unavailable rewind without changing the helper's phase.
pub(super) fn unavailable(app: &mut AppState, message: &str) {
    app.session.end_claude_edit();
    app.view.claude_edit.failed = true;
    let message = super::claude_question_state::safe_text(message);
    app.append_message(
        Role::System,
        format!("Could not edit Claude message: {message}. Draft kept."),
    );
    app.view.viewport.auto_follow_bottom = true;
}

/// Remember which queued text belongs to this edit until its SDK acknowledgement.
pub(super) fn queued(app: &mut AppState, text: &str) {
    app.session.queue_claude_edit(text);
    app.view.claude_edit.failed = false;
}

/// Move recovery responsibility from the local queue to the dispatched edit.
pub(super) fn dispatching(app: &mut AppState, text: &str, origin: PromptOrigin) {
    app.session.dispatch_claude_edit(text, origin);
}

/// Bind the exact human submission to the native response UUID.
pub(super) fn bind_submission(
    app: &mut AppState,
    text: &str,
    origin: PromptOrigin,
    response: &Value,
) {
    app.session.bind_claude_edit(text, origin, response);
}

/// Recover dispatched text while leaving an undispatched edit with its queue.
pub(super) fn recover_sending(app: &mut AppState) {
    app.session.recover_sending_claude_edit();
}

/// Recover a matching dequeued edit after a send failure.
pub(super) fn send_failed(app: &mut AppState, text: &str, message: &str) -> bool {
    if !app.session.reject_claude_edit_send(text) {
        return false;
    }
    failed(app, message);
    true
}

/// Clear the matching request or dismiss a prior local failure on a later send.
pub(super) fn accepted(app: &mut AppState, id: &str) {
    if app.view.claude_edit.failed {
        app.view.claude_edit.failed = false;
    } else if app.session.accept_claude_edit(id) {
        app.append_message(Role::System, "Claude started the edited branch".to_string());
    }
}

/// Stop automatic work and retain every queued human draft for explicit recovery.
pub(super) fn retire_queue(app: &mut AppState) {
    app.session.ralph_stop_after_failure();
    super::claude_session_history::retain_queued(app);
}
