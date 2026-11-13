//! Keep task-panel focus separate from native task facts.

use carlos_core::session::claude::{ClaudeTasks, TaskStatus};

/// Remember only the local panel and its explicit stop target.
#[derive(Default)]
pub(super) struct ClaudeTasksView {
    pub(super) show: bool,
    pub(super) scroll: usize,
    pub(super) selected: Option<String>,
}

impl ClaudeTasksView {
    /// Pin the first visible task once; a disappearing row never silently selects another stop target.
    pub(super) fn open(&mut self, tasks: &ClaudeTasks) {
        if self.selected.is_none() {
            self.selected = tasks
                .live()
                .keys()
                .next()
                .or_else(|| tasks.records().keys().next())
                .cloned();
        }
        self.show = true;
    }

    /// Keep the explicit selected identity even if native membership changes underneath it.
    pub(super) fn selected_id<'a>(&'a self, tasks: &ClaudeTasks) -> Option<&'a str> {
        self.selected
            .as_deref()
            .filter(|id| tasks.data(id).is_some())
    }
}

/// Render native task state without interpreting an acknowledgement as stopped.
pub(super) fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Paused => "paused",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Stopped => "stopped",
        TaskStatus::Unknown => "status unknown",
    }
}
