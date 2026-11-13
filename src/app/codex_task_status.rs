//! Apply local effects returned by the targeted native process reader.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::codex_panel::scope;
use super::state::AppState;
use crate::backend::BackendClient;
use crate::session::codex::task_status;

/// Read only known native process owners while background work remains active.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    task_status::tick(client, &mut app.session, scope, now)
}

/// Match native root, generation, revision and exact reader token before publishing.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    let scope = scope(
        app.session.codex_scope(),
        &app.session.codex_connection(),
        &app.view.codex_panel,
    );
    task_status::reply(&mut app.session, scope, params)
}

/// Wake only for actual pending native work and its original deadlines.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    task_status::wait(
        &app.session,
        scope(
            app.session.codex_scope(),
            &app.session.codex_connection(),
            &app.view.codex_panel,
        ),
        now,
    )
}
