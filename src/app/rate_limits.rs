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

    #[cfg(test)]
    pub(super) fn rate_limits_wait(&self, now: Instant) -> Option<Duration> {
        self.view
            .quota_next_draw
            .into_iter()
            .map(|deadline| deadline.saturating_duration_since(now))
            .chain(self.session.rate_limits().wait(now))
            .min()
    }

    #[cfg(test)]
    pub(super) fn clear_rate_limits(&mut self) {
        self.session.clear_rate_limits();
        self.view.quota_next_draw = None;
    }
}

#[cfg(test)]
#[path = "../tests/weekly_usage_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/weekly_usage_parity_tests.rs"]
mod parity_tests;
