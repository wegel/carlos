//! Compute backend wakeups without mixing them with terminal redraw timers.

use std::time::{Duration, Instant};

use super::codex::panel_scope::PanelScope;
use super::codex::{goals, panel, reconnect_streams, task_status};
use super::state::SessionState;

impl SessionState {
    /// Admit optional reads only after a native handshake has finished.
    pub(crate) fn backend_reads_ready(&self) -> bool {
        !self.codex_scope().enabled()
            || !self.codex_scope().closed() && !self.codex_connection().rebuilding()
    }

    /// Read the next backend deadline using only the caller's explicit panel targets.
    pub(crate) fn backend_wait(
        &self,
        now: Instant,
        scope: PanelScope<'_>,
        goal_visible: bool,
    ) -> Option<Duration> {
        self.pending_ralph_continuation_wait(now)
            .into_iter()
            .chain(
                self.backend_reads_ready()
                    .then(|| self.rate_limits().wait(now))
                    .flatten(),
            )
            .chain(self.claude().quota().wait(now))
            .chain(self.claude_context_wait(now))
            .chain(panel::wait(self, scope, now))
            .chain(task_status::wait(self, scope, now))
            .chain(goals::wait(self, self.codex_scope(), now, goal_visible))
            .chain(reconnect_streams::wait(self, now))
            .chain(
                (self.codex_connection().runtime_refresh()
                    && !self.codex_scope().closed()
                    && !self.codex_connection().runtime_read_pending())
                .then_some(Duration::ZERO),
            )
            .min()
    }
}
