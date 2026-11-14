//! Restore read interests and query private outcomes across transport changes without replaying writes.

use carlos_proto::commands::{Command, HelloResult};
use carlos_proto::submissions::OperationState;

use crate::client::{Client, Effects};
use crate::pending::Kind;
use crate::{ClientError, ConnectionId, Outbound, Welcome};

impl Client {
    /// Start a fresh socket generation and offer private retry rights in its greeting.
    pub fn begin(&mut self, name: String) -> Result<Outbound, ClientError> {
        let (connection, hello) = self.connection.begin(name)?;
        self.current = Some(connection);
        match self.requests.issue(connection, hello, Kind::Hello) {
            Ok(outgoing) => Ok(outgoing),
            Err(error) => {
                self.disconnect(connection);
                Err(error)
            }
        }
    }

    /// Retain private text and consistent cached facts after EOF; no request is automatically resent.
    pub fn disconnect(&mut self, connection: ConnectionId) {
        if self.current != Some(connection) {
            return;
        }
        self.connection.disconnect(connection);
        self.current = None;
        self.requests.clear();
        self.watches.disconnected();
        self.goals.disconnected();
        self.task_watches.disconnected();
        self.recover.clear();
        self.history = Default::default();
        for slot in self.sessions.values_mut() {
            slot.syncing = None;
            slot.subscribed = None;
        }
    }

    /// Distinguish an offline fact copy from a live greeted transport.
    pub fn is_ready(&self, connection: ConnectionId) -> bool {
        self.connection.is_ready(connection)
    }

    pub(crate) fn welcomed(
        &mut self,
        connection: ConnectionId,
        hello: HelloResult,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        let previous = self.connection.client().cloned();
        let welcome = self.connection.accept(connection, hello)?;
        if let Welcome::Replaced { instance_changed } = welcome {
            if let Some(client) = previous {
                self.operations.lose_rights(&client);
                effects.operations.extend(
                    self.operations
                        .iter()
                        .filter(|record| record.operation.client == client)
                        .map(|record| record.operation.clone()),
                );
            }
            self.bodies = Default::default();
            self.watches = Default::default();
            self.catalog = Default::default();
            effects.catalog = true;
            if instance_changed {
                for slot in self.sessions.values_mut() {
                    slot.replica = None;
                }
            }
        }
        if welcome == Welcome::Resumed {
            self.recover = self
                .operations
                .iter()
                .filter(|record| {
                    Some(&record.operation.client) == self.connection.client()
                        && matches!(
                            record.state,
                            OperationState::Pending
                                | OperationState::Queued
                                | OperationState::Uncertain { .. }
                        )
                })
                .map(|record| record.operation.clone())
                .collect();
        }
        self.restore_sessions(welcome);
        self.needs_watch = self.watching;
        effects.welcome = Some(welcome);
        Ok(())
    }

    fn restore_sessions(&mut self, welcome: Welcome) {
        for slot in self.sessions.values_mut() {
            slot.since = (welcome == Welcome::Resumed)
                .then(|| {
                    slot.replica
                        .as_ref()
                        .filter(|replica| !replica.is_stale())
                        .map(|replica| replica.sequence())
                })
                .flatten();
            slot.refresh = slot.wanted;
            slot.syncing = None;
            slot.subscribed = None;
        }
    }

    pub(crate) fn drive(&mut self, effects: &mut Effects) {
        let Some(connection) = self
            .current
            .filter(|connection| self.connection.is_ready(*connection))
        else {
            return;
        };
        self.drain_history(effects);
        if !self.drain_detach(effects) {
            return;
        }
        if self.needs_watch {
            match self.read(connection, Command::Watch {}) {
                Ok(outgoing) => effects.outgoing.push(outgoing),
                Err(_) => return,
            }
        }
        let sessions: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, slot)| slot.wanted && slot.refresh && slot.syncing.is_none())
            .map(|(id, slot)| (id.clone(), slot.since))
            .collect();
        for (session, since) in sessions {
            match self.read(connection, Command::Subscribe { session, since }) {
                Ok(outgoing) => effects.outgoing.push(outgoing),
                Err(_) => return,
            }
        }
        while let Some(operation) = self.recover.front().cloned() {
            match self.read(connection, Command::Operation { operation }) {
                Ok(outgoing) => {
                    self.recover.pop_front();
                    effects.outgoing.push(outgoing);
                }
                Err(_) => break,
            }
        }
        self.restore_watches(effects);
        self.restore_goal_watches(effects);
        self.restore_task_watches(effects);
    }
}
