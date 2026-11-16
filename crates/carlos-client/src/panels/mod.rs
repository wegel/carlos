//! Keep device-owned panel read windows separate from accepted session facts and UI editors.

mod queue;
pub(crate) mod queue_input;
mod queue_pages;
pub(crate) mod queue_reads;
pub(crate) mod settings;

pub use queue::{QueuePanel, QueueReadMode};
pub use queue_input::QueueInput;
pub use queue_pages::QueuePageWindow;
pub use settings::{SettingsCatalog, SettingsPanel};

use carlos_proto::ids::SessionId;

/// Name the shared model that changed without prescribing a platform's sheet or widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelKind {
    Queue,
    Settings,
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
    }

    pub(crate) fn disconnected(&mut self) {
        if let Some(queue) = &mut self.queue {
            queue.disconnected();
        }
        if let Some(settings) = &mut self.settings {
            settings.disconnected();
        }
    }
}

#[cfg(test)]
mod queue_pages_tests;
