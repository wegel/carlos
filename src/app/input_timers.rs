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
    connected(app) && app.rate_limits.tick(client, now)
}

fn connected(app: &AppState) -> bool {
    !app.codex.enabled || !app.codex.closed && !app.codex.connection.rebuilding
}

/// Choose the next usable read deadline without spinning on disconnected timers.
pub(in crate::app) fn idle_wait(app: &AppState, now: Instant) -> Option<std::time::Duration> {
    app.pending_ralph_continuation_wait(now)
        .into_iter()
        .chain(connected(app).then(|| app.rate_limits.wait(now)).flatten())
        .chain(app.claude.quota.wait(now))
        .chain(super::super::input_mode::wait(app, now))
        .chain(super::super::claude_context::wait(app, now))
        .chain(super::super::codex_panel::wait(app, now))
        .chain(super::super::codex_task_status::wait(app, now))
        .chain(super::super::codex_goals::wait(app, now))
        .chain(super::super::codex_reconnect_streams::wait(app, now))
        .chain(
            (app.codex.connection.runtime_refresh
                && !app.codex.closed
                && app.codex.connection.runtime_read.is_none())
            .then_some(std::time::Duration::ZERO),
        )
        .min()
}
