//! Bridge submitted Claude branch requests to local recovery and composer effects.

use super::state::AppState;

/// Report an accepted request independently of a local failure badge.
pub(super) fn waiting(app: &AppState) -> bool {
    app.session.claude_edit_progress().is_some()
}

/// Report an unavailable edit without reopening its helper.
pub(super) fn unavailable(app: &mut AppState, message: &str) {
    let mut update = Default::default();
    app.session.unavailable_claude_edit(&mut update, message);
    let _ = app.apply_claude_update(update);
}
