//! Keep Claude countdown redraw deadlines in the terminal view.

use std::time::{Duration, Instant};

use carlos_core::backend::{BackendClient, BackendKind};
use carlos_core::session::claude::ClaudePhase;

use super::state::AppState;

/// Redraw each minute and request optional quota only for a ready Claude helper.
pub(super) fn tick(app: &mut AppState, client: &dyn BackendClient, now: Instant) -> bool {
    if client.kind() != BackendKind::Claude || app.session.claude().phase() != ClaudePhase::Ready {
        return false;
    }
    let redraw = app
        .view
        .claude_quota_next_draw
        .is_none_or(|deadline| deadline <= now);
    if redraw {
        app.view.claude_quota_next_draw = Some(now + Duration::from_secs(60));
    }
    app.session.tick_claude_quota(client, now);
    redraw
}

/// Wake for the earlier countdown draw or optional backend read.
#[cfg(test)]
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    app.view
        .claude_quota_next_draw
        .into_iter()
        .map(|deadline| deadline.saturating_duration_since(now))
        .chain(app.session.claude().quota().wait(now))
        .min()
}
