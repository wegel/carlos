//! Schedule UI reads only after the current native connection has finished its handshake.

use std::time::Instant;

use super::super::AppState;
use crate::backend::BackendClient;

/// Keep optional quota reads out of initialization and failed connections.
pub(in crate::app) fn tick_rate_limits(
    client: &dyn BackendClient,
    app: &mut AppState,
    now: Instant,
) -> bool {
    app.session.backend_reads_ready() && app.tick_rate_limits(client, now)
}

/// Choose the next usable read deadline without spinning on disconnected timers.
pub(in crate::app) fn idle_wait(app: &AppState, now: Instant) -> Option<std::time::Duration> {
    let scope = super::super::codex_panel::scope(
        app.session.codex_scope(),
        app.session.codex_connection(),
        &app.view.codex_panel,
    );
    app.session
        .backend_wait(now, scope, app.view.codex_goals.show)
        .into_iter()
        .chain(
            app.session
                .backend_reads_ready()
                .then(|| {
                    app.view
                        .quota_next_draw
                        .map(|deadline| deadline.saturating_duration_since(now))
                })
                .flatten(),
        )
        .chain(
            app.view
                .claude_quota_next_draw
                .map(|deadline| deadline.saturating_duration_since(now)),
        )
        .chain(super::super::input_mode::wait(app, now))
        .min()
}
