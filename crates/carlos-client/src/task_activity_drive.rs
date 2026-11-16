//! Drive native child long polls and bounded display pages without a client timer.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;

use crate::panels::PanelKind;
use crate::{Client, Effects};

impl Client {
    pub(crate) fn drive_task_activities(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| {
                slot.panels
                    .task_activity
                    .as_ref()
                    .is_some_and(|panel| panel.dirty)
            })
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            if !self.drain_activity_closes(&session, effects) {
                continue;
            }
            let panel = self.task_activity_mut(&session).expect("activity model");
            panel.dirty = false;
            if !panel.wanted || panel.stale {
                continue;
            }
            self.request_activity_header(&session, effects);
            self.request_activity_page(&session, effects);
            self.enforce_fact_budget(&session, effects);
        }
    }

    fn drain_activity_closes(&mut self, session: &SessionId, effects: &mut Effects) -> bool {
        loop {
            let Some(activity) = self
                .task_activity(session)
                .and_then(|panel| panel.close.first())
                .cloned()
            else {
                return true;
            };
            let command = Command::TaskActivityClose {
                session: session.clone(),
                activity: Some(activity),
            };
            match self.read(self.current.expect("ready activity close"), command) {
                Ok(outgoing) => {
                    self.task_activity_mut(session)
                        .expect("activity model")
                        .close
                        .remove(0);
                    effects.outgoing.push(outgoing);
                }
                Err(_) => return false,
            }
        }
    }

    fn request_activity_header(&mut self, session: &SessionId, effects: &mut Effects) {
        let panel = self.task_activity(session).expect("open child");
        if panel.requests.header.is_some() {
            return;
        }
        let command = if let Some(header) = &panel.header {
            Command::TaskActivityRead {
                session: session.clone(),
                activity: header.activity.clone(),
                since: Some(header.change),
            }
        } else {
            let target = panel.target.as_ref().expect("captured child");
            Command::TaskActivityOpen {
                session: session.clone(),
                task: target.task.clone(),
                expected: target.expected.clone(),
                window: target.window.clone(),
            }
        };
        let result = self.read(self.current.expect("ready child"), command);
        let panel = self.task_activity_mut(session).expect("open child");
        match result {
            Ok(outgoing) => {
                panel.requests.header = Some(outgoing.id());
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                panel.error = Some(error.to_string());
                panel.close_current();
            }
        }
        effects.panel_changed(session, PanelKind::TaskActivity);
    }

    fn request_activity_page(&mut self, session: &SessionId, effects: &mut Effects) {
        let panel = self.task_activity_mut(session).expect("open child");
        if panel.stale || !panel.needs_page() {
            return;
        }
        panel.prepare_page();
        let header = panel.header.as_ref().expect("accepted child header");
        let after = panel
            .pending
            .as_ref()
            .and_then(|display| display.records.last())
            .map(|record| record.id.clone());
        let command = Command::TaskActivityPage {
            session: session.clone(),
            activity: header.activity.clone(),
            expected: header.revision.clone(),
            after,
            limit: Some(128),
        };
        let result = self.read(self.current.expect("ready child"), command);
        let panel = self.task_activity_mut(session).expect("open child");
        match result {
            Ok(outgoing) => {
                panel.requests.page = Some(outgoing.id());
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                panel.error = Some(error.to_string());
                panel.failed_revision = panel.pending.take().map(|display| display.revision);
            }
        }
        effects.panel_changed(session, PanelKind::TaskActivity);
    }
}
