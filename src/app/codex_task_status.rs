//! Apply local effects returned by the targeted native process reader.

use std::time::Instant;

use carlos_core::backend::BackendClient;
use carlos_core::session::codex::task_status;

use super::codex_panel::scope;
use super::state::AppState;

/// Read only known native process owners while background work remains active.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let scope = scope(
        app.session.codex_scope(),
        app.session.codex_connection(),
        &app.view.codex_panel,
    );
    task_status::tick(client, &mut app.session, scope, now)
}
