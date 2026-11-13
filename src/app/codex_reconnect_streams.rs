//! Apply partial native item redraw reports to the terminal.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::state::AppState;
use crate::backend::BackendClient;
use crate::session::codex::reconnect_streams;

/// Accept an exact native partial-item event before common dispatch.
pub(super) fn event(app: &mut AppState, event: &Value) -> bool {
    let mut changes = Default::default();
    let handled = reconnect_streams::event(&mut app.session, &mut changes, event);
    app.apply_item_changes(changes);
    handled
}
/// Read at most one partial native item at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let mut changes = Default::default();
    let changed = reconnect_streams::tick(client, &mut app.session, &mut changes, now);
    app.apply_item_changes(changes);
    changed
}
/// Apply one correlated partial-item read and its incremental redraw reports.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    let mut changes = Default::default();
    let handled = reconnect_streams::reply(&mut app.session, &mut changes, params);
    app.apply_item_changes(changes);
    handled
}
/// Wake only for the session's existing bounded partial-item deadlines.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    reconnect_streams::wait(&app.session, now)
}
