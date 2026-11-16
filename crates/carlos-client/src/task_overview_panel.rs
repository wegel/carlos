//! Drive owned overview pages and exact named text through the accepted session's task header.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;

use crate::panels::{PanelKind, TaskOverviewPanel};
use crate::{ChangeSet, Client, ClientError, Effects, ReadReply, ReadResult, RequestFailure};

impl Client {
    /// Open the latest task overview without selecting a task or changing a private draft.
    pub fn open_task_overview(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        let slot = self
            .sessions
            .get_mut(session)
            .filter(|slot| slot.wanted)
            .ok_or(ClientError::ForeignSession)?;
        if slot.panels.task_overview.is_none() {
            slot.panels.task_overview = Some(Box::new(TaskOverviewPanel {
                dirty: true,
                ..Default::default()
            }));
        }
        let mut effects = Effects::default();
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Borrow cached rows while the platform owns their formatting and quota repaint clock.
    pub fn task_overview(&self, session: &SessionId) -> Option<&TaskOverviewPanel> {
        self.sessions.get(session)?.panels.task_overview.as_deref()
    }

    /// Retire this reader's point requests, including while offline, without ending native work.
    pub fn close_task_overview(&mut self, session: &SessionId) -> Effects {
        self.cancel_task_overview_read(session);
        let mut effects = Effects::default();
        if let Some(slot) = self.sessions.get_mut(session) {
            if slot.panels.task_overview.take().is_some() {
                effects.panel_changed(session, PanelKind::TaskOverview);
            }
        }
        effects
    }

    fn task_overview_mut(&mut self, session: &SessionId) -> Option<&mut TaskOverviewPanel> {
        self.sessions
            .get_mut(session)?
            .panels
            .task_overview
            .as_deref_mut()
    }

    pub(crate) fn cancel_task_overview_read(&mut self, session: &SessionId) {
        let id = self
            .task_overview_mut(session)
            .and_then(|panel| panel.request.take());
        if let Some(id) = id {
            self.requests.take(&id);
        }
    }

    pub(crate) fn wake_task_overview(&mut self, session: &SessionId, changes: &ChangeSet) {
        let Some(slot) = self.sessions.get_mut(session) else {
            return;
        };
        let Some(panel) = &mut slot.panels.task_overview else {
            return;
        };
        panel.dirty |= changes.tasks
            || changes.reset
            || changes.info
            || panel.future.as_ref().is_some_and(|page| {
                slot.replica
                    .as_ref()
                    .is_some_and(|replica| replica.sequence() >= page.seq)
            });
    }

    pub(crate) fn drive_task_overviews(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| {
                slot.panels
                    .task_overview
                    .as_ref()
                    .is_some_and(|panel| panel.dirty)
            })
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            let Some(revision) = self
                .session(&session)
                .and_then(|replica| replica.tasks())
                .map(|facts| facts.overview_revision.clone())
            else {
                continue;
            };
            if self
                .task_overview(&session)
                .and_then(|panel| panel.revision.as_ref())
                != Some(&revision)
            {
                self.cancel_task_overview_read(&session);
                *self.task_overview_mut(&session).expect("open overview") = TaskOverviewPanel {
                    revision: Some(revision),
                    ..Default::default()
                };
                effects.panel_changed(&session, PanelKind::TaskOverview);
            }
            self.task_overview_mut(&session)
                .expect("open overview")
                .dirty = false;
            self.settle_task_overview(&session, effects);
            self.request_task_overview(&session, effects);
            self.enforce_fact_budget(&session, effects);
        }
    }

    fn settle_task_overview(&mut self, session: &SessionId, effects: &mut Effects) {
        let sequence = self.session(session).expect("current session").sequence();
        let panel = self.task_overview_mut(session).expect("open overview");
        let Some(page) = panel.future.take() else {
            return;
        };
        if sequence < page.seq {
            panel.future = Some(page);
            return;
        }
        panel.error = panel.accept_page(page).err().map(|error| error.to_string());
        effects.panel_changed(session, PanelKind::TaskOverview);
    }

    fn request_task_overview(&mut self, session: &SessionId, effects: &mut Effects) {
        let panel = self.task_overview(session).expect("open overview");
        if panel.error.is_some() || panel.request.is_some() || panel.future.is_some() {
            return;
        }
        let next = if panel.complete {
            panel.next_field()
        } else {
            Ok(None)
        };
        let text = match next {
            Ok(text) => text,
            Err(error) => {
                self.task_overview_mut(session)
                    .expect("open overview")
                    .error = Some(error.to_string());
                effects.panel_changed(session, PanelKind::TaskOverview);
                return;
            }
        };
        let expected = panel.revision.clone().expect("accepted overview revision");
        let command = if let Some(read) = &text {
            Command::TaskOverviewText {
                session: session.clone(),
                expected,
                entry: read.entry.clone(),
                field: read.field,
                offset: ByteOffset::new(read.offset as u64),
                limit: Some(panel.remaining_bytes().clamp(4, 65536) as u32),
            }
        } else if !panel.complete {
            Command::TaskOverview {
                session: session.clone(),
                expected: Some(expected),
                after: panel.entries.last().map(|entry| entry.id.clone()),
                limit: Some(32),
            }
        } else {
            return;
        };
        let result = self.read(self.current.expect("ready overview"), command);
        let panel = self.task_overview_mut(session).expect("open overview");
        match result {
            Ok(outgoing) => {
                panel.request = Some(outgoing.id());
                panel.text = text;
                effects.outgoing.push(outgoing);
            }
            Err(error) => panel.error = Some(error.to_string()),
        }
        effects.panel_changed(session, PanelKind::TaskOverview);
    }

    pub(crate) fn accept_task_overview_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let session = match &result.reply {
            ReadReply::TaskOverview(page) => &page.session,
            ReadReply::TaskOverviewText(text) => &text.session,
            _ => return Some(result),
        }
        .clone();
        let revision = self
            .session(&session)
            .and_then(|replica| replica.tasks())
            .map(|facts| facts.overview_revision.clone());
        let Some(panel) = self.task_overview_mut(&session) else {
            return Some(result);
        };
        if result.id.is_none() || panel.request != result.id {
            return Some(result);
        }
        panel.request = None;
        if panel.revision != revision {
            panel.text = None;
            panel.dirty = true;
            return None;
        }
        match result.reply {
            ReadReply::TaskOverview(page) => panel.future = Some(page),
            ReadReply::TaskOverviewText(text) => {
                let read = panel.text.take().expect("owned overview field");
                panel.error = panel
                    .accept_text(read, text)
                    .err()
                    .map(|error| error.to_string());
            }
            _ => unreachable!("owned overview reply"),
        }
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::TaskOverview);
        None
    }

    pub(crate) fn fail_task_overview_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let session = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .task_overview
                .as_ref()
                .filter(|panel| failure.id.is_some() && panel.request == failure.id)
                .map(|_| session.clone())
        });
        let Some(session) = session else { return false };
        let panel = self
            .task_overview_mut(&session)
            .expect("owned overview failure");
        panel.request = None;
        panel.text = None;
        panel.error = Some(failure.error.message.clone());
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::TaskOverview);
        true
    }
}
