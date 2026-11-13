//! Bridge submitted Claude branch requests to local recovery and composer effects.

#[cfg(test)]
use carlos_core::backend::PromptOrigin;
#[cfg(test)]
use serde_json::Value;

use super::state::AppState;

/// Keep the submitted edit until the fork and replacement startup are both ready.
#[cfg(test)]
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

/// Move recovery responsibility from the local queue to the dispatched edit.
#[cfg(test)]
pub(super) fn dispatching(app: &mut AppState, text: &str, origin: PromptOrigin) {
    app.session.dispatch_claude_edit(text, origin);
}

/// Bind the exact human submission to the native response UUID.
#[cfg(test)]
pub(super) fn bind_submission(
    app: &mut AppState,
    text: &str,
    origin: PromptOrigin,
    response: &Value,
) {
    app.session.bind_claude_edit(text, origin, response);
}

/// Recover a matching dequeued edit after a send failure.
#[cfg(test)]
pub(super) fn send_failed(app: &mut AppState, text: &str, message: &str) -> bool {
    if !app.session.reject_claude_edit_send(text) {
        return false;
    }
    failed(app, message);
    true
}

/// Restore failed submitted text through the session's recovery rules.
#[cfg(test)]
pub(super) fn failed(app: &mut AppState, message: &str) {
    let mut update = Default::default();
    app.session.fail_claude_edit(&mut update, message);
    let _ = app.apply_claude_update(update);
}

/// Report an unavailable edit without reopening its helper.
pub(super) fn unavailable(app: &mut AppState, message: &str) {
    let mut update = Default::default();
    app.session.unavailable_claude_edit(&mut update, message);
    let _ = app.apply_claude_update(update);
}

/// Stop automatic work and transfer queued human drafts once.
#[cfg(test)]
pub(super) fn retire_queue(app: &mut AppState) {
    let mut update = Default::default();
    app.session.retire_claude_queue(&mut update);
    let _ = app.apply_claude_update(update);
}

/// Apply the production acceptance rule independently of the local failure badge.
#[cfg(test)]
pub(super) fn accepted(app: &mut AppState, id: &str) {
    let mut update = Default::default();
    app.session.receive_claude_edit_acceptance(id, &mut update);
    let _ = app.apply_claude_update(update);
}
