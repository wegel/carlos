//! Apply partial native item redraw reports to the terminal.

#[cfg(test)]
use std::time::Duration;
use std::time::Instant;

use super::state::AppState;
use crate::backend::BackendClient;
use crate::session::codex::reconnect_streams;

/// Read at most one partial native item at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let mut changes = Default::default();
    let changed = reconnect_streams::tick(client, &mut app.session, &mut changes, now);
    app.apply_item_changes(changes);
    changed
}
/// Wake only for the session's existing bounded partial-item deadlines.
#[cfg(test)]
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    reconnect_streams::wait(&app.session, now)
}
