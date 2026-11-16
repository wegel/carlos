//! Send queued platform effects and choose visible metadata while Client owns common task reads.

use anyhow::Result;
use carlos_client::Effects;

use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn drive_tasks(&mut self, link: &mut Transport) -> Result<bool> {
        let changed = self.flush_task_effects(link)?;
        if let Err(error) = self.drive_task_stop(link) {
            self.tasks.view.error = Some(error.to_string());
            self.tasks.paint_dirty.set(true);
        }
        if !self.tasks.view.show
            || !self.client.session_is_ready(link.connection, &self.session)
            || self.tasks.activity.show && !self.task_activity_is_stale()
            || self.task_window_is_stale()
            || !self.task_pages_settled()
        {
            return Ok(changed);
        }
        self.drive_task_details(link)?;
        self.drive_task_overview(link)?;
        Ok(changed)
    }

    pub(super) fn apply_or_queue_task_effects(
        &mut self,
        link: Option<&mut Transport>,
        effects: Effects,
    ) -> Result<()> {
        if let Some(link) = link {
            self.flush_task_effects(link)?;
            self.apply_effects(link, effects)?;
        } else {
            self.tasks.effects.push(effects);
        }
        Ok(())
    }

    pub(super) fn flush_task_effects(&mut self, link: &mut Transport) -> Result<bool> {
        let mut changed = false;
        for effects in std::mem::take(&mut self.tasks.effects) {
            changed |= self.apply_effects(link, effects)?;
        }
        Ok(changed)
    }
}
