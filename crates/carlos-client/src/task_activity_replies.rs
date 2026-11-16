//! Settle owned child replies and close a retired opening by its exact returned reader identity.

use carlos_proto::ids::SessionId;

use crate::panels::PanelKind;
use crate::{Client, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    pub(crate) fn accept_task_activity_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let session = match &result.reply {
            ReadReply::TaskActivity(header) => &header.session,
            ReadReply::TaskActivityPage(page) => &page.session,
            _ => return Some(result),
        }
        .clone();
        let Some(id) = result.id.as_ref() else {
            return Some(result);
        };
        let Some(panel) = self.task_activity_mut(&session) else {
            return Some(result);
        };
        if let Some(index) = panel.retired.iter().position(|old| old == id) {
            panel.retired.remove(index);
            if let ReadReply::TaskActivity(header) = result.reply {
                panel.close.push(header.activity);
                panel.dirty = true;
            }
            return None;
        }
        match result.reply {
            ReadReply::TaskActivity(header) => {
                if panel.requests.header.as_ref() == Some(id) {
                    panel.requests.header = None;
                } else if panel.requests.advance.as_ref() == Some(id) {
                    panel.requests.advance = None;
                } else {
                    return Some(ReadResult {
                        id: result.id,
                        reply: ReadReply::TaskActivity(header),
                    });
                }
                panel.accept_header(header);
            }
            ReadReply::TaskActivityPage(page) => {
                if panel.requests.page.as_ref() != Some(id) {
                    return Some(ReadResult {
                        id: result.id,
                        reply: ReadReply::TaskActivityPage(page),
                    });
                }
                panel.requests.page = None;
                if let Err(error) = panel.accept_page(page) {
                    panel.failed_revision = panel.pending.take().map(|display| display.revision);
                    panel.error = Some(error.to_string());
                }
            }
            _ => unreachable!("owned child reply"),
        }
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::TaskActivity);
        None
    }

    pub(crate) fn fail_task_activity_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let Some(id) = failure.id.as_ref() else {
            return false;
        };
        let session: Option<SessionId> = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .task_activity
                .as_ref()
                .filter(|panel| {
                    panel.retired.contains(id)
                        || panel.requests.header.as_ref() == Some(id)
                        || panel.requests.page.as_ref() == Some(id)
                        || panel.requests.advance.as_ref() == Some(id)
                })
                .map(|_| session.clone())
        });
        let Some(session) = session else { return false };
        let panel = self
            .task_activity_mut(&session)
            .expect("owned child failure");
        if let Some(index) = panel.retired.iter().position(|old| old == id) {
            panel.retired.remove(index);
            return true;
        }
        let fatal = panel.requests.page.as_ref() != Some(id);
        if fatal {
            if panel.requests.header.as_ref() == Some(id) {
                panel.requests.header = None;
            }
            if panel.requests.advance.as_ref() == Some(id) {
                panel.requests.advance = None;
            }
            panel.close_current();
        } else {
            panel.requests.page = None;
            panel.failed_revision = panel.pending.take().map(|display| display.revision);
        }
        panel.error = Some(format!(
            "Child activity read failed; accepted display kept: {}",
            failure.error.message
        ));
        panel.dirty = true;
        if fatal {
            self.retire_task_activity_reads(&session);
        }
        effects.panel_changed(&session, PanelKind::TaskActivity);
        true
    }
}
