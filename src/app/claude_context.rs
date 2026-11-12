//! Supply native activity facts to the session-owned optional context reader.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::claude_state::ClaudePhase;
use super::state::AppState;
use crate::backend::BackendClient;

/// Request another snapshot without invalidating the last known count.
pub(super) fn refresh(app: &mut AppState) {
    app.session.refresh_claude_context();
}

/// Fence reads at the existing native context revision boundary.
pub(super) fn invalidate(app: &mut AppState) {
    app.claude.activity.context_revision += 1;
    app.session.invalidate_claude_context();
}

/// Send at most one optional summary read.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    let available = available(app);
    let working = working(app);
    app.session.tick_claude_context(
        client,
        now,
        available,
        working,
        &mut app.claude.activity.context_revision,
    )
}

/// Wake for pending reads and stale-count markers.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    app.session
        .claude_context_wait(now, available(app), working(app))
}

/// Settle the matching optional read before checking its context revision.
pub(super) fn receive(app: &mut AppState, frame: &Value, request: &Value) {
    app.session
        .receive_claude_context(frame, request, app.claude.activity.context_revision);
}

fn available(app: &AppState) -> bool {
    app.claude.enabled
        && app.session.claude().phase() == ClaudePhase::Ready
        && app.claude.activity.transient.as_deref() != Some("Compacting")
}

fn working(app: &AppState) -> bool {
    app.session.active_turn_id().is_some()
        || app.claude.activity.state != super::claude_activity::NativeState::Idle
}
