//! Settle owned goal reads through existing watch lifetimes and numbered session facts.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;

use crate::panels::{GoalPanel, PanelKind};
use crate::{
    ChangeSet, Client, ClientError, Effects, Outbound, ReadReply, ReadResult, RequestFailure,
};

impl Client {
    /// Open a goal reader using the daemon's existing native watch, without a client polling clock.
    pub fn open_goal_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        let slot = self
            .sessions
            .get_mut(session)
            .filter(|slot| slot.wanted)
            .ok_or(ClientError::ForeignSession)?;
        if slot.panels.goal.is_none() {
            slot.panels.goal = Some(Box::new(GoalPanel {
                refresh: true,
                dirty: true,
                ..Default::default()
            }));
        }
        let mut effects = self.watch_goal(session)?;
        effects.panel_changed(session, PanelKind::Goal);
        Ok(effects)
    }

    /// Explicitly refresh native facts, reusing an already pending fresh read when one exists.
    pub fn refresh_goal_panel(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        let panel = self
            .goal_panel_mut(session)
            .ok_or(ClientError::ForeignSession)?;
        panel.refresh = panel.read.is_none() && panel.future.is_none();
        panel.error = None;
        panel.dirty = true;
        let mut effects = self.watch_goal(session)?;
        effects.panel_changed(session, PanelKind::Goal);
        Ok(effects)
    }

    /// Borrow read status while rendering the session's single accepted goal copy.
    pub fn goal_panel(&self, session: &SessionId) -> Option<&GoalPanel> {
        self.sessions.get(session)?.panels.goal.as_deref()
    }

    /// Close the existing watch and retire its owned reply, including before its initial read ends.
    pub fn close_goal_panel(&mut self, session: &SessionId) -> Effects {
        let panel = self
            .sessions
            .get_mut(session)
            .and_then(|slot| slot.panels.goal.take());
        let Some(panel) = panel else {
            return Effects::default();
        };
        if let Some(id) = panel.read {
            self.requests.take(&id);
        }
        let mut effects = self.close_goal(session);
        effects.panel_changed(session, PanelKind::Goal);
        effects
    }

    pub(crate) fn goal_panel_mut(&mut self, session: &SessionId) -> Option<&mut GoalPanel> {
        self.sessions.get_mut(session)?.panels.goal.as_deref_mut()
    }

    pub(crate) fn cancel_goal_read(&mut self, session: &SessionId) {
        let id = self
            .goal_panel_mut(session)
            .and_then(|panel| panel.read.take());
        if let Some(id) = id {
            self.requests.take(&id);
        }
    }

    pub(crate) fn record_goal_panel_read(
        &mut self,
        session: &SessionId,
        outgoing: &Outbound,
        effects: &mut Effects,
    ) {
        if let Some(panel) = self.goal_panel_mut(session) {
            panel.read = Some(outgoing.id());
            panel.refresh = false;
            panel.error = None;
            panel.dirty = true;
            effects.panel_changed(session, PanelKind::Goal);
        }
    }

    pub(crate) fn wake_goal_panel(&mut self, session: &SessionId, changes: &ChangeSet) {
        let Some(slot) = self.sessions.get_mut(session) else {
            return;
        };
        let Some(panel) = &mut slot.panels.goal else {
            return;
        };
        let reached = panel.future.as_ref().is_some_and(|snapshot| {
            slot.replica
                .as_ref()
                .is_some_and(|replica| replica.sequence() >= snapshot.seq)
        });
        panel.dirty |= changes.goal || changes.reset || changes.info || reached;
    }

    pub(crate) fn drive_goal_panels(&mut self, effects: &mut Effects) {
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| slot.panels.goal.as_ref().is_some_and(|panel| panel.dirty))
            .map(|(session, _)| session.clone())
            .collect();
        for session in sessions {
            if !self.current_session_is_ready(&session) {
                continue;
            }
            self.goal_panel_mut(&session).expect("open goal").dirty = false;
            self.settle_goal_read(&session, effects);
            let next = self.goal_panel(&session).is_some_and(|panel| {
                panel.refresh && panel.read.is_none() && panel.future.is_none()
            });
            if next && self.goal_watch_is_active(&session) {
                self.request_goal_read(&session, effects);
            }
            self.enforce_fact_budget(&session, effects);
        }
    }

    fn settle_goal_read(&mut self, session: &SessionId, effects: &mut Effects) {
        let Some(snapshot) = self
            .goal_panel_mut(session)
            .and_then(|panel| panel.future.take())
        else {
            return;
        };
        if self
            .session(session)
            .is_some_and(|replica| replica.sequence() < snapshot.seq)
        {
            self.goal_panel_mut(session).expect("open goal").future = Some(snapshot);
            return;
        }
        match self.apply_goal_header(snapshot, effects) {
            Ok(()) | Err(ClientError::StaleGoal) => {}
            Err(error) => {
                if let Some(panel) = self.goal_panel_mut(session) {
                    panel.error = Some(error.to_string());
                }
            }
        }
        effects.panel_changed(session, PanelKind::Goal);
    }

    fn request_goal_read(&mut self, session: &SessionId, effects: &mut Effects) {
        self.goal_panel_mut(session).expect("open goal").refresh = false;
        match self.read(
            self.current.expect("ready goal reader"),
            Command::GoalRead {
                session: session.clone(),
                refresh: true,
            },
        ) {
            Ok(outgoing) => {
                self.record_goal_panel_read(session, &outgoing, effects);
                effects.outgoing.push(outgoing);
            }
            Err(error) => {
                self.goal_panel_mut(session).expect("open goal").error = Some(error.to_string());
                effects.panel_changed(session, PanelKind::Goal);
            }
        }
    }

    pub(crate) fn accept_goal_panel_reply(
        &mut self,
        result: ReadResult,
        effects: &mut Effects,
    ) -> Option<ReadResult> {
        let ReadReply::Goal(snapshot) = &result.reply else {
            return Some(result);
        };
        let session = snapshot.session.clone();
        let Some(panel) = self.goal_panel_mut(&session) else {
            return Some(result);
        };
        if result.id.is_none() || panel.read != result.id {
            return Some(result);
        }
        let ReadReply::Goal(snapshot) = result.reply else {
            unreachable!("goal reply")
        };
        panel.read = None;
        panel.error = None;
        panel.future = Some(snapshot);
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::Goal);
        None
    }

    pub(crate) fn fail_goal_panel_read(
        &mut self,
        failure: &RequestFailure,
        effects: &mut Effects,
    ) -> bool {
        let Some(session) = self.sessions.iter().find_map(|(session, slot)| {
            slot.panels
                .goal
                .as_ref()
                .filter(|panel| panel.read == failure.id && failure.id.is_some())
                .map(|_| session.clone())
        }) else {
            return false;
        };
        let panel = self.goal_panel_mut(&session).expect("matching goal reader");
        panel.read = None;
        panel.error = Some(failure.error.message.clone());
        panel.dirty = true;
        effects.panel_changed(&session, PanelKind::Goal);
        true
    }
}
