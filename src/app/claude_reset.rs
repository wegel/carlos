//! Exercise native clear and init reports through the client fixture.

use serde_json::Value;

use super::state::AppState;

/// Apply a native clear through the production session owner.
pub(super) fn receive(app: &mut AppState, message: &Value) {
    let mut update = Default::default();
    app.session.receive_claude_reset(message, &mut update);
    let _ = app.apply_claude_update(update);
}

/// Apply an init through the production session owner.
pub(super) fn initialize(app: &mut AppState, message: &Value) {
    let mut update = Default::default();
    app.session.receive_claude_initialized(message, &mut update);
    let _ = app.apply_claude_update(update);
}
