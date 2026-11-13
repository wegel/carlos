//! Exercise Claude callback client reports without duplicating session commands.

use carlos_core::session::claude::receive_update::ClaudeDraftContext;
use serde_json::Value;

use super::claude_notifications::context;
use super::state::AppState;

/// Create the local editor described by the accepted session callback.
pub(super) fn receive_callback(app: &mut AppState, frame: &Value) {
    let received = app.session.receive_claude_callback(frame);
    app.apply_claude_callback(received);
}

/// Remove only the cancelled editor and its matching local consent token.
pub(super) fn cancel_callback(app: &mut AppState, id: &Value) {
    let draft = ClaudeDraftContext {
        composer: app.view.input.lines(),
        ..context(app)
    };
    let mut update = Default::default();
    app.session
        .receive_claude_cancelled(id, &draft, &mut update);
    let _ = app.apply_claude_update(update);
}
