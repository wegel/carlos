//! Apply partial native item redraw reports to the terminal.

use std::time::Instant;

use carlos_core::backend::BackendClient;
use carlos_core::session::codex::reconnect_streams;

use super::state::AppState;

/// Read at most one partial native item at the existing timer point.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let changes = Default::default();
    let changed = reconnect_streams::tick(client, &mut app.session, now);
    app.apply_item_changes(changes);
    changed
}
