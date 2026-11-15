//! Admit explicit device commands and remember only the targets needed to match their replies.

use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;
use carlos_proto::submissions::{ClientOperation, RecoveryKind};

use crate::client::{Client, Slot};
use crate::pending::Kind;
use crate::{BodyToken, ClientError, ConnectionId, LocalText, Outbound};

impl Client {
    /// Retain a failed write's exact text and close its transport generation without replaying work.
    pub fn write_failed(
        &mut self,
        connection: ConnectionId,
        id: carlos_proto::commands::RequestId,
        message: &str,
        possibly_written: bool,
    ) -> crate::Effects {
        let mut effects = crate::Effects::default();
        if self.current != Some(connection) {
            return effects;
        }
        if let Some((_, Kind::Mutation(operation))) = self.requests.take(&id) {
            if self
                .operations
                .fail_local(&operation, message, possibly_written)
            {
                effects.operations.push(operation);
            }
        }
        self.disconnect(connection);
        effects.failures.push(crate::RequestFailure {
            id: Some(id),
            error: carlos_proto::commands::RpcError {
                code: -32012,
                message: message.chars().take(256).collect(),
            },
        });
        effects
    }

    /// Allocate an operation only for explicit new work, and retain its exact text before writing.
    pub fn submit(
        &mut self,
        connection: ConnectionId,
        build: impl FnOnce(ClientOperation) -> Command,
    ) -> Result<Outbound, ClientError> {
        self.require_ready(connection)?;
        let operation = self.connection.next_operation(connection)?;
        let command = build(operation.clone());
        let (origin, session) = mutation(&command)
            .ok_or_else(|| ClientError::InvalidPayload("submit requires a mutation".into()))?;
        if origin != &operation {
            return Err(ClientError::InvalidConnection);
        }
        let text = submitted_text(&command)?;
        self.operations
            .begin(operation.clone(), session.cloned(), text)?;
        match self
            .requests
            .issue(connection, command, Kind::Mutation(operation.clone()))
        {
            Ok(outgoing) => Ok(outgoing),
            Err(error) => {
                self.operations
                    .fail_local(&operation, &error.to_string(), false);
                Err(error)
            }
        }
    }

    /// Issue the next bounded cached-body read and attach its exact local lifetime to the reply.
    pub fn read_body(
        &mut self,
        connection: ConnectionId,
        token: &BodyToken,
    ) -> Result<Outbound, ClientError> {
        self.require_ready(connection)?;
        let command = self.bodies.request(token)?;
        let mut kind = crate::pending::read_kind(&command)?;
        match &mut kind {
            Kind::Source(target) | Kind::Output(target) => {
                target.token = Some(token.clone());
            }
            _ => unreachable!("body read command"),
        }
        self.requests.issue(connection, command, kind)
    }

    pub(crate) fn check_interest(&self, kind: &Kind) -> Result<(), ClientError> {
        if let Kind::History { session, .. } = kind {
            self.require_history_window(session)?;
        }
        if let Kind::Subscribe(session) = kind {
            if !self.sessions.contains_key(session) && self.sessions.len() >= 32 {
                return Err(ClientError::Capacity("client session windows are full"));
            }
        }
        if let Kind::GoalWatch(session) | Kind::TaskWatch(session) = kind {
            if !self.sessions.get(session).is_some_and(|slot| slot.wanted) {
                return Err(ClientError::ForeignSession);
            }
        }
        if let Kind::Operation(operation) = kind {
            if Some(&operation.client) != self.connection.client() {
                return Err(ClientError::InvalidConnection);
            }
        }
        Ok(())
    }

    pub(crate) fn record_interest(&mut self, outgoing: &Outbound) {
        self.record_goal_watch(outgoing);
        self.record_task_watch(outgoing);
        match self
            .requests
            .kind(outgoing.id)
            .expect("just issued request")
            .clone()
        {
            Kind::Watch => {
                self.watching = true;
                self.needs_watch = false;
            }
            Kind::Subscribe(session) => {
                let slot = self.sessions.entry(session).or_insert(Slot {
                    replica: None,
                    wanted: true,
                    syncing: None,
                    subscribed: None,
                    refresh: false,
                    since: None,
                });
                slot.wanted = true;
                slot.syncing = Some(outgoing.id);
                slot.subscribed = None;
                slot.refresh = false;
            }
            Kind::Unsubscribe(session) => {
                self.sessions.remove(&session);
                self.goals.forget(&session);
                self.task_watches.forget(&session);
                self.bodies.forget_session(&session);
            }
            Kind::BodyUnsubscribe {
                session,
                entry,
                source,
            } => self.watches.unsubscribed(session, entry, source),
            _ => {}
        }
    }
}

fn mutation(command: &Command) -> Option<(&ClientOperation, Option<&SessionId>)> {
    Some(match command {
        Command::Create { operation, .. } => (operation, None),
        Command::Send {
            operation, session, ..
        }
        | Command::Rename {
            operation, session, ..
        }
        | Command::Rewind {
            operation, session, ..
        }
        | Command::SetRalph {
            operation, session, ..
        }
        | Command::Approve {
            operation, session, ..
        }
        | Command::Interrupt {
            operation, session, ..
        }
        | Command::GoalSet {
            operation, session, ..
        }
        | Command::GoalClear {
            operation, session, ..
        }
        | Command::ToolLabelsSet {
            operation, session, ..
        }
        | Command::ContextSet {
            operation, session, ..
        }
        | Command::SettingsSet {
            operation, session, ..
        }
        | Command::QueueChange {
            operation, session, ..
        }
        | Command::TaskStop {
            operation, session, ..
        } => (operation, Some(session)),
        _ => return None,
    })
}

fn submitted_text(command: &Command) -> Result<Option<LocalText>, ClientError> {
    let (kind, text) = match command {
        Command::Send { text, .. } => (RecoveryKind::Prompt, text.as_str()),
        Command::Rewind { text, .. } => (RecoveryKind::Edit, text.as_str()),
        Command::QueueChange { change, .. } => match change.text() {
            Some(text) => (RecoveryKind::Queued, text),
            None => return Ok(None),
        },
        _ => return Ok(None),
    };
    if text.len() > 512 * 1024 {
        return Err(ClientError::Capacity("submitted text exceeds 512 KiB"));
    }
    Ok(Some(LocalText {
        kind,
        text: text.into(),
    }))
}
