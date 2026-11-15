//! Drive session-owned task commands from existing terminal integration fixtures.

use carlos_core::session::codex::panel_receive;
use carlos_core::session::codex::panel_scope::PanelUpdate;
use carlos_core::session::codex::panel_state::Request;

use crate::app::codex_panel::{apply, scope};
use crate::app::state::AppState;

pub(super) fn failed(app: &mut AppState, request: &Request, error: &str) {
    let scope = scope(
        app.session.codex_scope(),
        app.session.codex_connection(),
        &app.view.codex_panel,
    );
    let mut update = PanelUpdate::default();
    panel_receive::failed(&mut app.session, scope, &mut update, request, error);
    apply(app, update);
}
