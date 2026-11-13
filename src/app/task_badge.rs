//! Count native background work without treating completed rows as active tasks.

use super::state::AppState;

/// Read session-owned task facts independently of panel visibility.
pub(super) fn count(app: &AppState) -> usize {
    carlos_core::session::codex::task_badge::count(
        &app.session,
        super::codex_panel::scope(
            app.session.codex_scope(),
            app.session.codex_connection(),
            &app.view.codex_panel,
        ),
    )
}

/// Check exact native ownership without consulting a display label.
pub(super) fn owns(app: &AppState, owner: &str) -> bool {
    carlos_core::session::codex::task_badge::owns(
        &app.session,
        super::codex_panel::scope(
            app.session.codex_scope(),
            app.session.codex_connection(),
            &app.view.codex_panel,
        ),
        owner,
    )
}
