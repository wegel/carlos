//! Settle only owned detail replies against their original task, window, revision and exact range.

use carlos_proto::ids::SessionId;

use crate::panels::task_details::MAX_TASK_DETAIL_BYTES;
use crate::panels::PanelKind;
use crate::{Client, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    pub(crate) fn accept_task_detail_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let session = match &result.reply {
            ReadReply::TaskDetail(value) => &value.session,
            ReadReply::TaskText(text) => &text.session,
            _ => return Some(result),
        }
        .clone();
        let Some(panel) = self.task_details(&session) else {
            return Some(result);
        };
        if result.id.is_none() || panel.pending.as_ref().map(|read| &read.id) != result.id.as_ref()
        {
            return Some(result);
        }
        let current = self.task_detail_target_is_current(&session);
        let panel = self.task_details_mut(&session).expect("owned task detail");
        let read = panel.pending.take().expect("owned task read");
        if !current {
            return None;
        }
        let outcome = match result.reply {
            ReadReply::TaskDetail(value) => {
                let bytes: usize = value
                    .properties
                    .iter()
                    .map(|property| property.text.len())
                    .sum();
                if read.field.is_some()
                    || value.task.id != panel.target.task
                    || value.revision != panel.target.expected
                    || bytes > MAX_TASK_DETAIL_BYTES
                    || value
                        .properties
                        .iter()
                        .any(|property| property.bytes.get() > MAX_TASK_DETAIL_BYTES as u64)
                {
                    Err(crate::ClientError::InvalidPayload(
                        "Task detail changed its target or exceeded the native 8 MiB view".into(),
                    ))
                } else {
                    panel.value = Some(value);
                    Ok(())
                }
            }
            ReadReply::TaskText(text) => panel.accept_text(read, text),
            _ => unreachable!("owned detail reply"),
        };
        panel.error = outcome.err().map(|error| error.to_string());
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::TaskDetails);
        None
    }

    pub(crate) fn fail_task_detail_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let session: Option<SessionId> = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .task_details
                .as_ref()
                .filter(|panel| {
                    failure.id.is_some()
                        && panel.pending.as_ref().map(|read| &read.id) == failure.id.as_ref()
                })
                .map(|_| session.clone())
        });
        let Some(session) = session else { return false };
        let sequence = self
            .session(&session)
            .map(|replica| replica.task_sequence());
        let panel = self.task_details_mut(&session).expect("owned task failure");
        let read = panel.pending.take().expect("owned task request");
        if sequence.is_none_or(|sequence| read.sequence >= sequence) {
            panel.error = Some(failure.error.message.clone());
            panel.failed_sequence = Some(read.sequence);
        }
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::TaskDetails);
        true
    }
}
