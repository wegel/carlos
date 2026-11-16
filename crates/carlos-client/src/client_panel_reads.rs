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
                ReadReply::ContextDefaults(_) => self.accept_context_panel_reply(result, effects),
                ReadReply::Goal(_) => self.accept_goal_panel_reply(result, effects),
                ReadReply::TaskDetail(_) | ReadReply::TaskText(_) => {
                    self.accept_task_detail_reply(result, effects)
                }
                ReadReply::TaskOverview(_) | ReadReply::TaskOverviewText(_) => {
                    self.accept_task_overview_reply(result, effects)
                }
                ReadReply::TaskActivity(_) | ReadReply::TaskActivityPage(_) => {
                    self.accept_task_activity_reply(result, effects)
                }
                ReadReply::TaskSnapshot(_) | ReadReply::TaskPage(_) | ReadReply::TaskWindow(_) => {
                    self.accept_task_panel_reply(result, effects)
                }
                ReadReply::Devices(_)
                | ReadReply::PromptSelection(_)
                | ReadReply::PromptText(_)
                | ReadReply::SavedPage(_)
                | ReadReply::TaskRun(_)
                | ReadReply::Source { .. }
                | ReadReply::Output { .. } => Some(result),
            };
            if let Some(result) = unowned {
                effects.replies.push(result);
            }
        }
        for failure in std::mem::take(&mut effects.failures) {
            if !(self.fail_queue_panel_read(&failure, effects)
                || self.fail_settings_panel_read(&failure, effects)
                || self.fail_context_panel_read(&failure, effects)
                || self.fail_goal_panel_read(&failure, effects)
                || self.fail_task_detail_read(&failure, effects)
                || self.fail_task_overview_read(&failure, effects)
                || self.fail_task_activity_read(&failure, effects)
                || self.fail_task_panel_read(&failure, effects))
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
        self.wake_goal_panel(session, changes);
        self.wake_task_details(session, changes);
        self.wake_task_overview(session, changes);
        self.wake_task_panel(session, changes);
    }

    pub(crate) fn drive_panels(&mut self, effects: &mut Effects) {
        self.drive_queue_panels(effects);
        self.drive_settings_panels(effects);
        self.drive_context_panels(effects);
        self.drive_goal_panels(effects);
        self.drive_task_details(effects);
        self.drive_task_overviews(effects);
        self.drive_task_activities(effects);
        self.drive_task_panels(effects);
    }

    pub(crate) fn cancel_panel_reads(&mut self, session: &SessionId) {
        self.cancel_queue_reads(session);
        self.cancel_settings_read(session);
        self.cancel_context_read(session);
        self.cancel_goal_read(session);
        self.cancel_task_detail_read(session);
        self.cancel_task_overview_read(session);
        self.cancel_task_activity_reads(session);
        self.cancel_task_panel_reads(session);
    }
}
