//! Activate body watches only after their first reply and keep older resets away from replacement reads.

use std::collections::{HashMap, VecDeque};

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::ids::{EntryId, SessionId};

use crate::client::{Client, Effects, RequestFailure};
use crate::pending::{self, Kind};
use crate::{BodyToken, ClientError, ConnectionId, Outbound};

#[derive(Clone, PartialEq, Eq, Hash)]
struct Key {
    session: SessionId,
    entry: EntryId,
    source: bool,
}

impl Key {
    fn token(token: &BodyToken) -> Self {
        Self {
            session: token.session().clone(),
            entry: token.entry().clone(),
            source: token.is_source(),
        }
    }
    fn unsubscribe(&self) -> Command {
        if self.source {
            Command::SourceUnsubscribe {
                session: self.session.clone(),
                entry: self.entry.clone(),
            }
        } else {
            Command::OutputUnsubscribe {
                session: self.session.clone(),
                entry: self.entry.clone(),
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct Watches {
    desired: HashMap<Key, BodyToken>,
    active: HashMap<Key, BodyToken>,
    cancel: VecDeque<Key>,
}

impl Watches {
    pub(crate) fn unsubscribed(&mut self, session: SessionId, entry: EntryId, source: bool) {
        let key = Key {
            session,
            entry,
            source,
        };
        self.desired.remove(&key);
        self.active.remove(&key);
    }
    pub(crate) fn disconnected(&mut self) {
        self.active.clear();
        self.cancel.clear();
    }
}

impl Client {
    /// Start a body watch from its accepted cursor; first complete a direct seek when needed.
    pub fn watch_body(
        &mut self,
        connection: ConnectionId,
        token: &BodyToken,
    ) -> Result<Outbound, ClientError> {
        self.require_ready(connection)?;
        let key = Key::token(token);
        if !self.watches.desired.contains_key(&key)
            && (self.watches.desired.len() >= 32 || self.watches.cancel.len() >= 32)
        {
            return Err(ClientError::Capacity(
                "body watches are full; drain pending cancellations first",
            ));
        }
        let command = match self.bodies.request(token)? {
            Command::Source {
                session,
                entry,
                cursor,
                ..
            } => Command::SourceSubscribe {
                session,
                entry,
                cursor,
            },
            Command::Output {
                session,
                entry,
                cursor,
                ..
            } => Command::OutputSubscribe {
                session,
                entry,
                cursor,
            },
            _ => {
                return Err(ClientError::InvalidPayload(
                    "complete the first direct source seek before watching".into(),
                ))
            }
        };
        let mut kind = pending::read_kind(&command)?;
        match &mut kind {
            Kind::Source(target) | Kind::Output(target) => target.token = Some(token.clone()),
            _ => unreachable!("typed body watch"),
        }
        let outgoing = self.requests.issue(connection, command, kind)?;
        self.watches.desired.insert(key, token.clone());
        Ok(outgoing)
    }

    /// Drop one local expansion and queue a bounded unsubscribe without disturbing another format.
    pub fn close_body(&mut self, token: &BodyToken) -> Effects {
        let key = Key::token(token);
        if self.watches.desired.get(&key) == Some(token) {
            self.watches.desired.remove(&key);
            self.watches.active.remove(&key);
            self.cancel_watch(key);
        }
        self.bodies.close(token);
        let mut effects = Effects::default();
        self.drive(&mut effects);
        effects
    }

    pub(crate) fn activate_watch(&mut self, token: &BodyToken) {
        let key = Key::token(token);
        if self.watches.desired.get(&key) == Some(token) && self.bodies.contains(token) {
            self.watches.active.insert(key, token.clone());
        }
    }

    pub(crate) fn failed_watch(&mut self, token: &BodyToken) {
        let key = Key::token(token);
        if self.watches.desired.get(&key) == Some(token) {
            self.watches.desired.remove(&key);
            self.watches.active.remove(&key);
            self.bodies.close(token);
            self.cancel_watch(key);
        }
    }

    pub(crate) fn source_watch(
        &mut self,
        update: carlos_proto::source::SourceUpdate,
        effects: &mut Effects,
    ) {
        let key = Key {
            session: update.session,
            entry: update.chunk.entry.clone(),
            source: true,
        };
        let Some(token) = self.watches.active.get(&key).cloned() else {
            return;
        };
        let result = self.bodies.apply_source(&token, update.chunk);
        self.watch_result(key, token, result, effects);
    }

    pub(crate) fn output_watch(
        &mut self,
        update: carlos_proto::output::OutputUpdate,
        effects: &mut Effects,
    ) {
        let key = Key {
            session: update.session,
            entry: update.chunk.entry.clone(),
            source: false,
        };
        let Some(token) = self.watches.active.get(&key).cloned() else {
            return;
        };
        let result = self.bodies.apply_output(&token, update.chunk);
        self.watch_result(key, token, result, effects);
    }

    fn watch_result(
        &mut self,
        key: Key,
        token: BodyToken,
        result: Result<bool, ClientError>,
        effects: &mut Effects,
    ) {
        if result == Err(ClientError::RetiredBody)
            && self
                .watches
                .desired
                .get(&key)
                .is_some_and(|current| current != &token)
        {
            self.watches.active.remove(&key);
            return;
        }
        match result {
            Ok(true) => effects.bodies.push(token),
            Ok(false) => {}
            Err(error) => {
                effects.bodies.push(token.clone());
                self.failed_watch(&token);
                self.watches.active.remove(&key);
                self.cancel_watch(key);
                if error != ClientError::RetiredBody {
                    failure(error.to_string(), effects);
                }
            }
        }
    }

    pub(crate) fn reset_body_watch(
        &mut self,
        session: SessionId,
        entry: EntryId,
        source: bool,
        message: String,
        effects: &mut Effects,
    ) {
        let key = Key {
            session,
            entry,
            source,
        };
        let Some(token) = self.watches.active.remove(&key) else {
            return;
        };
        self.bodies.close(&token);
        if self.watches.desired.get(&key) == Some(&token) {
            self.watches.desired.remove(&key);
        }
        effects.bodies.push(token);
        failure(message, effects);
    }

    pub(crate) fn restore_watches(&mut self, effects: &mut Effects) {
        let Some(connection) = self.current else {
            return;
        };
        let retired: Vec<_> = self
            .watches
            .desired
            .values()
            .filter(|token| !self.bodies.contains(token))
            .cloned()
            .collect();
        for token in retired {
            self.replace_watch(token, effects);
        }
        while let Some(key) = self.watches.cancel.front().cloned() {
            match self
                .requests
                .issue(connection, key.unsubscribe(), Kind::Unit)
            {
                Ok(outgoing) => {
                    self.watches.cancel.pop_front();
                    effects.outgoing.push(outgoing);
                }
                Err(_) => return,
            }
        }
        let desired: Vec<_> = self
            .watches
            .desired
            .iter()
            .filter(|(key, token)| {
                !self.watches.active.contains_key(*key)
                    && !self.requests.has_watch(token)
                    && self.session_is_ready(connection, token.session())
            })
            .map(|(_, token)| token.clone())
            .collect();
        for token in desired {
            match self.watch_body(connection, &token) {
                Ok(outgoing) => effects.outgoing.push(outgoing),
                Err(_) => break,
            }
        }
    }

    fn replace_watch(&mut self, token: BodyToken, effects: &mut Effects) {
        let key = Key::token(&token);
        self.watches.desired.remove(&key);
        self.watches.active.remove(&key);
        self.cancel_watch(key.clone());
        let revision = self
            .session(&key.session)
            .and_then(|replica| replica.entry(&key.entry))
            .and_then(|entry| {
                if key.source {
                    entry
                        .source
                        .as_ref()
                        .map(|source| source.revision)
                        .or_else(|| entry.tool.as_ref().map(|tool| tool.output_revision))
                } else {
                    entry.tool.as_ref().map(|tool| tool.output_revision)
                }
            });
        let Some(revision) = revision else {
            return;
        };
        let replacement = if key.source && token.document_source() {
            self.bodies
                .open_source_document(key.session.clone(), key.entry.clone(), revision)
        } else if key.source {
            self.bodies
                .open_source(key.session.clone(), key.entry.clone(), revision)
        } else {
            self.bodies
                .open_output(key.session.clone(), key.entry.clone(), revision, None)
        };
        match replacement {
            Ok(token) => {
                effects.bodies.push(token.clone());
                self.watches.desired.insert(key, token);
            }
            Err(error) => failure(error.to_string(), effects),
        }
    }

    fn cancel_watch(&mut self, key: Key) {
        if !self.watches.cancel.contains(&key) {
            self.watches.cancel.push_back(key);
        }
    }
}

fn failure(message: String, effects: &mut Effects) {
    effects.failures.push(RequestFailure {
        id: None,
        error: RpcError {
            code: -32012,
            message: message.chars().take(256).collect(),
        },
    });
}
