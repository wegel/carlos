//! Restore explicit goal reads across reconnects and retire them independently of session facts.

use std::collections::{HashMap, HashSet};

use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::SessionId;

use crate::pending::Kind;
use crate::{Client, ClientError, Effects, Outbound};

#[derive(Default)]
pub(crate) struct GoalWatches {
    wanted: HashSet<SessionId>,
    active: HashMap<SessionId, i64>,
    cancelled: HashSet<SessionId>,
}

impl GoalWatches {
    pub(crate) fn disconnected(&mut self) {
        self.active.clear();
        self.cancelled.clear();
    }

    pub(crate) fn forget(&mut self, session: &SessionId) {
        self.wanted.remove(session);
        self.active.remove(session);
        self.cancelled.remove(session);
    }
    pub(crate) fn failed(&mut self, session: &SessionId, serial: i64) {
        if self.active.get(session) == Some(&serial) {
            self.forget(session);
            self.cancelled.insert(session.clone());
        }
    }
}

impl Client {
    pub(crate) fn goal_watch_is_active(&self, session: &SessionId) -> bool {
        self.goals.active.contains_key(session)
    }

    /// Identify this session's outstanding goal subscribe read without exposing transport internals.
    pub fn pending_goal_read(&self, session: &SessionId) -> Option<RequestId> {
        let serial = *self.goals.active.get(session)?;
        matches!(self.requests.kind(serial), Some(Kind::GoalWatch(_)))
            .then_some(RequestId::Integer(serial))
    }

    /// Start a bounded goal read after this session's facts return, including when opened offline.
    pub fn watch_goal(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        if !self.wants_session(session) {
            return Err(ClientError::ForeignSession);
        }
        self.goals.wanted.insert(session.clone());
        self.goals.cancelled.remove(session);
        let mut effects = Effects::default();
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Release a device's goal panel, including while offline, without ending the session.
    pub fn close_goal(&mut self, session: &SessionId) -> Effects {
        self.goals.wanted.remove(session);
        if self.goals.active.remove(session).is_some() {
            self.goals.cancelled.insert(session.clone());
        }
        let mut effects = Effects::default();
        self.drive(&mut effects);
        effects
    }

    pub(crate) fn record_goal_watch(&mut self, outgoing: &Outbound) {
        match self.requests.kind(outgoing.id) {
            Some(Kind::GoalWatch(session)) => {
                self.goals.wanted.insert(session.clone());
                self.goals.active.insert(session.clone(), outgoing.id);
                self.goals.cancelled.remove(session);
            }
            Some(Kind::GoalUnwatch(session)) => self.goals.forget(session),
            _ => {}
        }
    }

    pub(crate) fn restore_goal_watches(&mut self, effects: &mut Effects) {
        let Some(connection) = self.current else {
            return;
        };
        let cancelled: Vec<_> = self.goals.cancelled.iter().cloned().collect();
        for session in cancelled {
            match self.read(connection, Command::GoalUnsubscribe { session }) {
                Ok(outgoing) => effects.outgoing.push(outgoing),
                Err(_) => return,
            }
        }
        let wanted: Vec<_> = self
            .goals
            .wanted
            .iter()
            .filter(|id| !self.goals.active.contains_key(*id))
            .filter(|id| {
                self.sessions.get(*id).is_some_and(|slot| {
                    slot.wanted
                        && slot.subscribed == self.current
                        && !slot.refresh
                        && slot.syncing.is_none()
                        && slot
                            .replica
                            .as_ref()
                            .is_some_and(|replica| !replica.is_stale())
                })
            })
            .cloned()
            .collect();
        for session in wanted {
            match self.read(
                connection,
                Command::GoalSubscribe {
                    session: session.clone(),
                },
            ) {
                Ok(outgoing) => {
                    self.record_goal_panel_read(&session, &outgoing, effects);
                    effects.outgoing.push(outgoing);
                }
                Err(_) => return,
            }
        }
    }
}
