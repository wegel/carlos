//! Apply local editor resets around the session's native clear and init fences.

use serde_json::Value;

use super::state::AppState;

/// Apply each native reset once while preserving callbacks and process tasks.
pub(super) fn receive(app: &mut AppState, message: &Value) {
    if !app.session.begin_claude_reset(message) {
        return;
    }
    let old_id = app.session.thread_id().clone();
    app.reset_preserving_approvals(old_id);
    app.session.set_thread_name(None);
    app.session.clear_claude_history();
    app.view.claude_unknown_selection = None;
    app.view.claude_edit = Default::default();
    app.session.finish_claude_reset();
}

/// Adopt only a string native identity while the session awaits its reset init.
pub(super) fn initialize(app: &mut AppState, message: &Value) {
    if let Some(id) = app.session.claude_reset_identity(message) {
        app.set_thread_id(id);
        app.session.finish_claude_reset_init();
    }
}
