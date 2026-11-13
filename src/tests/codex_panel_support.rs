//! Drive session-owned task commands from existing terminal integration fixtures.

use serde_json::Value;

use crate::app::codex_panel::{apply, scope};

use crate::app::state::AppState;
use carlos_core::session::codex::panel_receive;
use carlos_core::session::codex::panel_scope::PanelUpdate;
use carlos_core::session::codex::panel_state::Request;
use carlos_core::session::codex::task_status;

pub(super) fn observed(app: &mut AppState, method: &str, params: &Value) {
    let scope = scope(
        app.session.codex_scope(),
        app.session.codex_connection(),
        &app.view.codex_panel,
    );
    task_status::observed(&mut app.session, scope, method, params);
}
pub(super) fn panel_updated(app: &mut AppState) {
    let scope = scope(
        app.session.codex_scope(),
        app.session.codex_connection(),
        &app.view.codex_panel,
    );
    task_status::panel_updated(&mut app.session, scope);
}
pub(super) fn terminated(app: &mut AppState, owner: &str, process: &str, item: Option<&str>) {
    task_status::terminated(&mut app.session, owner, process, item);
}
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
