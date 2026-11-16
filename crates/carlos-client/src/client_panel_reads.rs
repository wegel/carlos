//! Route owned panel replies and update their cached byte charges without consuming explicit reads.

use carlos_proto::ids::SessionId;

use crate::{ChangeSet, Client, Effects, ReadReply};

impl Client {
    pub(crate) fn accept_panel_replies(&mut self, effects: &mut Effects) {
        for result in std::mem::take(&mut effects.replies) {
            let unowned = match &result.reply {
                ReadReply::Queue(_) | ReadReply::QueueText(_) => {
                    self.accept_queue_panel_reply(result, effects)
                }
                ReadReply::Settings(_) => self.accept_settings_panel_reply(result, effects),
                _ => Some(result),
            };
            if let Some(result) = unowned {
                effects.replies.push(result);
            }
        }
        for failure in std::mem::take(&mut effects.failures) {
            if !(self.fail_queue_panel_read(&failure, effects)
                || self.fail_settings_panel_read(&failure, effects))
            {
                effects.failures.push(failure);
            }
        }
        let sessions: Vec<_> = effects
            .panels
            .iter()
            .map(|change| change.session.clone())
            .collect();
        for session in sessions {
            self.enforce_fact_budget(&session, effects);
        }
    }

    pub(crate) fn wake_panels(&mut self, session: &SessionId, changes: &ChangeSet) {
        self.wake_queue_panel(session, changes);
        self.wake_settings_panel(session, changes);
    }

    pub(crate) fn drive_panels(&mut self, effects: &mut Effects) {
        self.drive_queue_panels(effects);
        self.drive_settings_panels(effects);
    }

    pub(crate) fn cancel_panel_reads(&mut self, session: &SessionId) {
        self.cancel_queue_reads(session);
        self.cancel_settings_read(session);
    }
}
