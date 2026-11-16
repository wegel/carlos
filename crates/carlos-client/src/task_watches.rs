//! Restore explicit task reads across reconnects and retire them independently of session facts.

use std::collections::{HashMap, HashSet};

use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::SessionId;

use crate::pending::Kind;
use crate::{Client, ClientError, Effects, Outbound};

#[derive(Default)]
pub(crate) struct TaskWatches {
    wanted: HashSet<SessionId>,
    active: HashMap<SessionId, i64>,
    cancelled: HashSet<SessionId>,
}

impl TaskWatches {
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
    pub(crate) fn task_watch_is_active(&self, session: &SessionId) -> bool {
        self.task_watches.active.contains_key(session)
    }

    pub(crate) fn release_task_watch(&mut self, session: &SessionId) {
        self.task_watches.wanted.remove(session);
        if self.task_watches.active.remove(session).is_some() {
            self.task_watches.cancelled.insert(session.clone());
        }
    }

    /// Identify this session's outstanding task subscribe read without exposing transport internals.
    pub fn pending_task_read(&self, session: &SessionId) -> Option<RequestId> {
        let serial = *self.task_watches.active.get(session)?;
        matches!(self.requests.kind(serial), Some(Kind::TaskWatch(_)))
            .then_some(RequestId::Integer(serial))
    }

    /// Start a bounded task read after this session's facts return, including when opened offline.
    pub fn watch_tasks(&mut self, session: &SessionId) -> Result<Effects, ClientError> {
        if !self.wants_session(session) {
            return Err(ClientError::ForeignSession);
        }
        self.task_watches.wanted.insert(session.clone());
        self.task_watches.cancelled.remove(session);
        let mut effects = Effects::default();
        self.drive(&mut effects);
        Ok(effects)
    }

    /// Release a device's task panel, including while offline, without ending the session.
    pub fn close_tasks(&mut self, session: &SessionId) -> Effects {
        self.release_task_watch(session);
        let mut effects = Effects::default();
        self.drive(&mut effects);
        effects
    }

    pub(crate) fn record_task_watch(&mut self, outgoing: &Outbound) {
        match self.requests.kind(outgoing.id) {
            Some(Kind::TaskWatch(session)) => {
                self.task_watches.wanted.insert(session.clone());
                self.task_watches
                    .active
                    .insert(session.clone(), outgoing.id);
                self.task_watches.cancelled.remove(session);
            }
            Some(Kind::TaskUnwatch(session)) => self.task_watches.forget(session),
            _ => {}
        }
    }

    pub(crate) fn restore_task_watches(&mut self, effects: &mut Effects) {
        let Some(connection) = self.current else {
            return;
        };
        let cancelled: Vec<_> = self.task_watches.cancelled.iter().cloned().collect();
        for session in cancelled {
            match self.read(connection, Command::TaskUnsubscribe { session }) {
                Ok(outgoing) => effects.outgoing.push(outgoing),
                Err(_) => return,
            }
        }
        let wanted: Vec<_> = self
            .task_watches
            .wanted
            .iter()
            .filter(|id| !self.task_watches.active.contains_key(*id))
            .filter(|id| self.session_is_ready(connection, id))
            .cloned()
            .collect();
        for session in wanted {
            match self.read(
                connection,
                Command::TaskSubscribe {
                    session: session.clone(),
                },
            ) {
                Ok(outgoing) => {
                    self.record_task_panel_read(&session, &outgoing, effects);
                    effects.outgoing.push(outgoing);
                }
                Err(_) => return,
            }
        }
    }
}
