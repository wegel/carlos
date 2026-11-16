//! Keep device-owned panel read windows separate from accepted session facts and UI editors.

mod context;
mod goal;
mod queue;
pub(crate) mod queue_input;
mod queue_pages;
pub(crate) mod queue_reads;
pub(crate) mod settings;
pub(crate) mod task_details;
mod task_move;
mod task_pages;

pub use context::ContextPanel;
pub use goal::GoalPanel;
pub use queue::{QueuePanel, QueueReadMode};
pub use queue_input::QueueInput;
pub use queue_pages::QueuePageWindow;
pub use settings::{SettingsCatalog, SettingsPanel};
pub use task_details::{TaskDetailTarget, TaskDetails, TaskFieldRead, MAX_TASK_DETAIL_BYTES};
pub use task_pages::TaskPageWindow;

use carlos_proto::ids::SessionId;

/// Name the shared model that changed without prescribing a platform's sheet or widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Queue,
    Settings,
    Context,
    Goal,
    TaskDetails,
}

/// A platform redraws only the panel whose checked data or read status changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PanelChange {
    pub session: SessionId,
    pub kind: PanelKind,
}

#[derive(Default)]
pub(crate) struct Panels {
    pub queue: Option<Box<QueuePanel>>,
    pub settings: Option<Box<SettingsPanel>>,
    pub context: Option<Box<ContextPanel>>,
    pub goal: Option<Box<GoalPanel>>,
    pub task_details: Option<Box<TaskDetails>>,
}

impl Panels {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.queue
            .as_ref()
            .map_or(0, |queue| queue.retained_bytes())
            + self
                .settings
                .as_ref()
                .map_or(0, |settings| settings.retained_bytes())
            + self
                .context
                .as_ref()
                .map_or(0, |context| context.retained_bytes())
            + self.goal.as_ref().map_or(0, |goal| goal.retained_bytes())
            + self
                .task_details
                .as_ref()
                .map_or(0, |details| details.retained_bytes())
    }

    pub(crate) fn disconnected(&mut self) {
        if let Some(queue) = &mut self.queue {
            queue.disconnected();
        }
        if let Some(settings) = &mut self.settings {
            settings.disconnected();
        }
        if let Some(context) = &mut self.context {
            context.disconnected();
        }
        if let Some(goal) = &mut self.goal {
            goal.disconnected();
        }
        if let Some(details) = &mut self.task_details {
            details.disconnected();
        }
    }
}

#[cfg(test)]
mod queue_pages_tests;
