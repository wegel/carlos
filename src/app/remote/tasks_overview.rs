//! Redraw terminal quota countdowns while the shared client owns overview reads and long labels.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn drive_task_overview(&mut self, link: &mut Transport) -> Result<()> {
        if self.client.task_overview(&self.session).is_none() {
            let effects = self.client.open_task_overview(&self.session)?;
            self.apply_effects(link, effects)?;
        }
        Ok(())
    }

    pub(super) fn tick_task_countdowns(&mut self) -> bool {
        let now = Instant::now();
        let changed = self.tasks.view.show
            && self
                .client
                .task_overview(&self.session)
                .is_some_and(|overview| overview.has_quota())
            && self
                .tasks
                .view
                .overview_next_draw
                .is_none_or(|deadline| deadline <= now);
        if changed {
            self.tasks.view.overview_next_draw = Some(now + Duration::from_secs(60));
            self.tasks.paint_dirty.set(true);
        }
        changed
    }
}
