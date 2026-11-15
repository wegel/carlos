//! Wake the daemon owner after each worker result while preserving the embedded caller's sender.

use std::sync::mpsc::{SendError, Sender, SyncSender};

use super::SummaryEvent;

pub(super) enum Output {
    Embedded(Sender<SummaryEvent>),
    Owner {
        sender: SyncSender<SummaryEvent>,
        wake: SyncSender<()>,
    },
}

impl Output {
    pub(super) fn send(&self, event: SummaryEvent) -> Result<(), SendError<SummaryEvent>> {
        match self {
            Self::Embedded(sender) => sender.send(event),
            Self::Owner { sender, wake } => {
                sender.send(event)?;
                let _ = wake.try_send(());
                Ok(())
            }
        }
    }
}
