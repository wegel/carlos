//! Keep terminal child visibility, scroll and frame order while Client owns native reader state.

use anyhow::{ensure, Result};
use carlos_client::panels::TaskDetailTarget;
use carlos_proto::tasks::TaskKind;

use super::transport::Transport;
use super::Device;

#[derive(Default)]
pub(super) struct ChildActivity {
    pub(super) show: bool,
}

impl Device {
    pub(super) fn task_activity_is_stale(&self) -> bool {
        self.client
            .task_activity(&self.session)
            .is_none_or(|activity| activity.is_stale())
    }

    pub(super) fn open_task_activity(&mut self, link: Option<&mut Transport>) -> Result<()> {
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Reconnect before reading child activity"))?;
        ensure!(
            self.client.session_is_ready(link.connection, &self.session),
            "Wait for accepted session facts before reading a child"
        );
        let selected = self
            .selected_task()
            .ok_or_else(|| anyhow::anyhow!("Select an available child first"))?;
        if selected.kind != TaskKind::Agent {
            return Ok(());
        }
        if self.tasks.activity.show
            && self
                .client
                .task_activity(&self.session)
                .is_some_and(|activity| !activity.is_stale() && activity.header().is_some())
        {
            return self.advance_task_activity(link, false);
        }
        ensure!(
            self.task_view_is_current(),
            "Wait for the selected task's accepted page before opening activity"
        );
        let target = TaskDetailTarget {
            task: selected.id.clone(),
            expected: self
                .task_page_revision()
                .cloned()
                .expect("accepted task revision"),
            window: self.task_window_id(),
        };
        self.flush_task_effects(link)?;
        self.tasks.activity.show = true;
        self.tasks.view.scroll = 0;
        self.tasks.paint_dirty.set(true);
        let effects = self.client.pause_task_panel(&self.session, true)?;
        self.apply_effects(link, effects)?;
        let effects = self.client.open_task_activity(&self.session, target)?;
        self.apply_effects(link, effects)?;
        Ok(())
    }

    pub(super) fn advance_task_activity(
        &mut self,
        link: &mut Transport,
        older: bool,
    ) -> Result<()> {
        ensure!(
            !self.task_activity_is_stale(),
            "This child reader retired; Enter explicitly reopens its selected task"
        );
        let effects = self.client.advance_task_activity(&self.session, older)?;
        self.tasks.view.scroll = 0;
        self.tasks.paint_dirty.set(true);
        self.apply_effects(link, effects)?;
        Ok(())
    }

    pub(super) fn close_task_activity(&mut self, restore_list: bool) -> Result<()> {
        self.tasks.activity.show = false;
        let effects = self.client.close_task_activity(&self.session);
        self.tasks.effects.push(effects);
        self.tasks.view.scroll = 0;
        self.tasks.paint_dirty.set(true);
        if self.client.task_panel(&self.session).is_some() {
            let effects = self.client.pause_task_panel(&self.session, !restore_list)?;
            self.tasks.effects.push(effects);
        }
        Ok(())
    }
}
