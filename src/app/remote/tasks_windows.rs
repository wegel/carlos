//! Adapt readonly task-window getters and the explicit older-window key to the shared client.

use anyhow::Result;
use carlos_proto::ids::{TaskId, TaskRevision, TaskWindowId};
use carlos_proto::session::Backend;
use carlos_proto::task_windows::TaskWindow;
use carlos_proto::tasks::Task;

use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn selected_task(&self) -> Option<&Task> {
        self.client.task_panel(&self.session)?.selected()
    }

    pub(super) fn selected_task_id(&self) -> Option<&TaskId> {
        self.client.task_panel(&self.session)?.selected_id()
    }

    pub(super) fn task_page_revision(&self) -> Option<&TaskRevision> {
        self.client.task_panel(&self.session)?.revision()
    }

    pub(super) fn task_position(&self) -> Option<usize> {
        self.client.task_panel(&self.session)?.pages().position()
    }

    pub(super) fn task_pages_settled(&self) -> bool {
        self.client
            .task_panel(&self.session)
            .is_some_and(|panel| panel.pages().is_settled())
    }

    pub(super) fn task_window(&self) -> Option<&TaskWindow> {
        self.client.task_panel(&self.session)?.window()
    }

    pub(super) fn task_window_id(&self) -> Option<TaskWindowId> {
        self.task_window().map(|window| window.window.clone())
    }

    pub(super) fn task_view_is_current(&self) -> bool {
        self.client.task_panel_is_current(&self.session)
    }

    pub(super) fn task_window_is_stale(&self) -> bool {
        self.client
            .task_panel(&self.session)
            .is_some_and(|panel| panel.window_is_stale())
    }

    pub(super) fn older_tasks_reading(&self) -> bool {
        self.client
            .task_panel(&self.session)
            .is_some_and(|panel| panel.is_reading_older())
    }

    pub(super) fn open_older_tasks(&mut self, link: Option<&mut Transport>) -> Result<()> {
        if self.task_backend() != Some(Backend::Codex) {
            return Ok(());
        }
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Reconnect before reading older native tasks"))?;
        self.flush_task_effects(link)?;
        let effects = self.client.open_older_task_window(&self.session)?;
        self.tasks.view.error = None;
        self.tasks.paint_dirty.set(true);
        self.apply_effects(link, effects)?;
        Ok(())
    }
}
