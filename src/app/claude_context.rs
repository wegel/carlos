//! Supply native activity facts to the session-owned optional context reader.

use std::time::{Duration, Instant};

#[cfg(test)]
use serde_json::Value;

use super::state::AppState;
use crate::backend::BackendClient;

/// Request another snapshot without invalidating the last known count.
#[cfg(test)]
pub(super) fn refresh(app: &mut AppState) {
    app.session.refresh_claude_context();
}

/// Fence reads at the existing native context revision boundary.
pub(super) fn invalidate(app: &mut AppState) {
    app.session.invalidate_claude_context();
}

/// Send at most one optional summary read.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    app.session.tick_claude_context(client, now)
}

/// Wake for pending reads and stale-count markers.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    app.session.claude_context_wait(now)
}

/// Settle the matching optional read before checking its context revision.
#[cfg(test)]
pub(super) fn receive(app: &mut AppState, frame: &Value, request: &Value) {
    app.session.receive_claude_context(frame, request);
}
