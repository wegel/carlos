//! Apply local effects after the session matches a native panel reply.

use serde_json::Value;

use super::codex_panel::{apply, scope};
use super::state::AppState;
use crate::session::codex::panel_receive;
use crate::session::codex::panel_scope::PanelUpdate;

/// Settle the exact native reader/control and apply only its reported local changes.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    let scope = scope(
        app.session.codex_scope(),
        &app.codex.connection,
        &app.view.codex_panel,
    );
    let mut update = PanelUpdate::default();
    let handled = panel_receive::reply(&mut app.session, scope, &mut update, params);
    apply(app, update);
    handled
}
