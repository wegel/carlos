//! Settle owned task headers, pages and candidate windows without consuming independent reads.

use carlos_proto::ids::SessionId;

use crate::panels::tasks::Failure;
use crate::panels::PanelKind;
use crate::{Client, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    pub(crate) fn accept_task_panel_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let Some(session) = task_reply_session(&result.reply) else {
            return Some(result);
        };
        let Some(id) = result.id.as_ref() else {
            return Some(result);
        };
        let Some(panel) = self.task_panel_mut(&session) else {
            return Some(result);
        };
        if let Some(index) = panel.windows.retired.iter().position(|old| old == id) {
            panel.windows.retired.remove(index);
            if let ReadReply::TaskWindow(header) = result.reply {
                panel.windows.close.push(header.window);
                panel.dirty = true;
            }
            return None;
        }
        let window_owned = panel
            .windows
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.as_ref() == Some(id));
        match result.reply {
            ReadReply::TaskSnapshot(snapshot) if panel.reads.header.as_ref() == Some(id) => {
                panel.reads.header = None;
                panel.reads.header_error = None;
                panel.reads.future_header = Some(snapshot);
            }
            ReadReply::TaskPage(page) if window_owned => {
                self.accept_task_window_page(&session, page, effects)
            }
            ReadReply::TaskPage(page)
                if panel.reads.page.as_ref().is_some_and(|read| &read.id == id) =>
            {
                let read = panel.reads.page.take().expect("owned task page");
                panel.reads.future_page = Some((read, page));
            }
            ReadReply::TaskWindow(header) if window_owned => {
                panel.accept_window_header(header);
            }
            reply => {
                return Some(ReadResult {
                    id: result.id,
                    reply,
                })
            }
        }
        self.task_panel_mut(&session)
            .expect("owned task panel")
            .dirty = true;
        effects.panel_changed(&session, PanelKind::Tasks);
        None
    }

    pub(crate) fn fail_task_panel_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let Some(id) = failure.id.as_ref() else {
            return false;
        };
        let session: Option<SessionId> = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .tasks
                .as_ref()
                .filter(|panel| {
                    panel.windows.retired.contains(id)
                        || panel.reads.header.as_ref() == Some(id)
                        || panel.reads.page.as_ref().is_some_and(|read| &read.id == id)
                        || panel
                            .windows
                            .pending
                            .as_ref()
                            .is_some_and(|pending| pending.request.as_ref() == Some(id))
                })
                .map(|_| session.clone())
        });
        let Some(session) = session else { return false };
        let sequence = self
            .session(&session)
            .map(|replica| replica.task_sequence());
        let panel = self.task_panel_mut(&session).expect("owned task failure");
        if let Some(index) = panel.windows.retired.iter().position(|old| old == id) {
            panel.windows.retired.remove(index);
            return true;
        }
        if panel
            .windows
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.as_ref() == Some(id))
        {
            self.fail_task_window_candidate(&session, &failure.error.message);
        } else if panel.reads.header.as_ref() == Some(id) {
            panel.reads.header = None;
            panel.reads.header_error = Some(failure.error.message.clone());
        } else {
            let read = panel.reads.page.take().expect("owned failed page");
            if sequence.is_none_or(|current| read.sequence >= current) {
                panel.reads.failure = Some(Failure {
                    sequence: read.sequence,
                    text: failure.error.message.clone(),
                });
            }
        }
        self.task_panel_mut(&session).expect("open tasks").dirty = true;
        effects.panel_changed(&session, PanelKind::Tasks);
        true
    }
}

fn task_reply_session(reply: &ReadReply) -> Option<SessionId> {
    match reply {
        ReadReply::TaskSnapshot(snapshot) => Some(snapshot.session.clone()),
        ReadReply::TaskPage(page) => Some(page.session.clone()),
        ReadReply::TaskWindow(header) => Some(header.session.clone()),
        _ => None,
    }
}
