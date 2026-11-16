//! Consume only queue replies and failures owned by the shared panel controller.

use crate::panels::PanelKind;
use crate::{Client, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    pub(crate) fn accept_queue_panel_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let session = match &result.reply {
            ReadReply::Queue(page) => &page.session,
            ReadReply::QueueText(text) => &text.session,
            _ => return Some(result),
        }
        .clone();
        let Some(panel) = self.queue_panel_mut(&session) else {
            return Some(result);
        };
        if matches!(&result.reply, ReadReply::QueueText(_)) {
            if !panel
                .input
                .read
                .as_ref()
                .is_some_and(|read| read.request == result.id && result.id.is_some())
            {
                return Some(result);
            }
            let ReadReply::QueueText(text) = result.reply else {
                unreachable!("input reply")
            };
            self.accept_queue_input(text, effects);
            return None;
        }
        if !panel
            .reads
            .request
            .as_ref()
            .is_some_and(|read| Some(&read.id) == result.id.as_ref())
        {
            return Some(result);
        }
        let read = panel.reads.request.take().expect("matching queue page");
        let ReadReply::Queue(page) = result.reply else {
            unreachable!("queue reply")
        };
        panel.reads.future = Some((read, page));
        panel.dirty = true;
        panel.recount();
        effects.panel_changed(&session, PanelKind::Queue);
        None
    }

    pub(crate) fn fail_queue_panel_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let input = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .queue
                .as_ref()
                .filter(|panel| {
                    panel
                        .input
                        .read
                        .as_ref()
                        .is_some_and(|read| read.request == failure.id && failure.id.is_some())
                })
                .map(|_| session.clone())
        });
        if let Some(session) = input {
            self.fail_queue_input(&session, failure.error.message.clone());
            effects.panel_changed(&session, PanelKind::Queue);
            return true;
        }
        let Some(session) = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .queue
                .as_ref()
                .filter(|panel| {
                    panel
                        .reads
                        .request
                        .as_ref()
                        .is_some_and(|read| Some(&read.id) == failure.id.as_ref())
                })
                .map(|_| session.clone())
        }) else {
            return false;
        };
        let panel = self
            .queue_panel_mut(&session)
            .expect("matching queue panel");
        let read = panel.reads.request.take().expect("matching failed read");
        self.queue_read_failed(&session, read.revision, failure.error.message.clone());
        effects.panel_changed(&session, PanelKind::Queue);
        true
    }
}
