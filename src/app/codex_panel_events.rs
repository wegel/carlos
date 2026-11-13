//! Keep child events out of the root transcript after the session records their facts.

use serde_json::Value;

use super::codex_panel::scope;
use super::state::AppState;
use crate::session::codex::panel_events;

/// Record native task facts before the app applies its root-thread filter.
pub(super) fn event(app: &mut AppState, method: &str, params: &Value) -> bool {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    panel_events::event(&mut app.session, scope, method, params)
}
