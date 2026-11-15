//! Keep the terminal countdown timer separate from backend quota read deadlines.

use std::time::{Duration, Instant};

use carlos_core::backend::{BackendClient, BackendKind};

use super::state::AppState;

const DISPLAY_INTERVAL: Duration = Duration::from_secs(60);

impl AppState {
    pub(super) fn tick_rate_limits(&mut self, client: &dyn BackendClient, now: Instant) -> bool {
        if client.kind() != BackendKind::Codex {
            return false;
        }
        let redraw = self
            .view
            .quota_next_draw
            .is_none_or(|deadline| deadline <= now);
        if redraw {
            self.view.quota_next_draw = Some(now + DISPLAY_INTERVAL);
        }
        self.session.tick_rate_limits(client, now);
        redraw
    }
}
