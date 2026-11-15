//! Supply native activity facts to the session-owned optional context reader.

use std::time::Instant;

use carlos_core::backend::BackendClient;

use super::state::AppState;

/// Fence reads at the existing native context revision boundary.
pub(super) fn invalidate(app: &mut AppState) {
    app.session.invalidate_claude_context();
}

/// Send at most one optional summary read.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    app.session.tick_claude_context(client, now)
}
