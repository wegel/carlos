//! Decode replies by their original typed request without dispatching native backend methods.

use carlos_proto::commands::{RpcError, Sessions};
use carlos_proto::submissions::OperationResult;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::client::{Client, Effects, ReadReply, ReadResult};
use crate::pending::Kind;
use crate::{ClientError, ConnectionId};

impl Client {
    pub(crate) fn success(
        &mut self,
        connection: ConnectionId,
        serial: i64,
        kind: Kind,
        value: Value,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        if !matches!(&kind, Kind::Hello) {
            self.require_ready(connection)?;
        }
        match kind {
            Kind::Hello => self.welcomed(connection, parse(value)?, effects)?,
            Kind::Watch => {
                let list: Sessions = parse(value)?;
                self.catalog.replace(list.sessions)?;
                effects.catalog = true;
            }
            Kind::Subscribe(session) => self.subscribed(serial, session, parse(value)?, effects)?,
            Kind::History {
                session,
                before,
                limit,
            } => {
                let page: carlos_proto::session::History = parse(value)?;
                if page.entries.len() > limit {
                    return Err(ClientError::InvalidPayload(
                        "history reply exceeds the requested row count".into(),
                    ));
                }
                self.history_reply(serial, session, before, page, effects);
            }
            Kind::Mutation(operation) | Kind::Operation(operation) => {
                let result: OperationResult = parse(value)?;
                if result.operation != operation {
                    return Err(ClientError::InvalidPayload(
                        "private result has another operation".into(),
                    ));
                }
                if self.operations.apply(result)? {
                    effects.operations.push(operation);
                }
            }
            kind => self.panel_reply(serial, kind, value, effects)?,
        }
        Ok(())
    }

    fn panel_reply(
        &mut self,
        serial: i64,
        kind: Kind,
        value: Value,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        let reply = match kind {
            Kind::Goal(session) | Kind::GoalWatch(session) => {
                let goal: carlos_proto::goals::GoalSnapshot = parse(value)?;
                same_session(&session, &goal.session)?;
                ReadReply::Goal(goal)
            }
            Kind::Queue { session, expected } => {
                ReadReply::Queue(self.queue_page_reply(session, expected, value)?)
            }
            Kind::QueueText(target) => ReadReply::QueueText(self.queue_text_reply(target, value)?),
            Kind::Source(target) => return self.source_reply(serial, target, value, effects),
            Kind::Output(target) => return self.output_reply(serial, target, value, effects),
            Kind::Unit
            | Kind::GoalUnwatch(_)
            | Kind::Unsubscribe(_)
            | Kind::BodyUnsubscribe { .. } => {
                let _: () = parse(value)?;
                return Ok(());
            }
            _ => unreachable!("non-panel reply handled first"),
        };
        effects.replies.push(ReadResult {
            id: Some(carlos_proto::commands::RequestId::Integer(serial)),
            reply,
        });
        Ok(())
    }

    fn source_reply(
        &mut self,
        serial: i64,
        target: crate::pending::BodyTarget,
        value: Value,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        let chunk: carlos_proto::source::SourceChunk = parse(value)?;
        crate::body_apply::source_bounds(&chunk)?;
        if chunk.entry != target.entry {
            return Err(ClientError::InvalidBody);
        }
        if let Some(token) = target.token {
            let result = self.bodies.apply_source(&token, chunk);
            return self.body_reply(serial, token, target.watch, result, effects);
        }
        effects.replies.push(ReadResult {
            id: Some(carlos_proto::commands::RequestId::Integer(serial)),
            reply: ReadReply::Source {
                session: target.session,
                chunk,
            },
        });
        Ok(())
    }

    fn output_reply(
        &mut self,
        serial: i64,
        target: crate::pending::BodyTarget,
        value: Value,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        let chunk: carlos_proto::output::OutputChunk = parse(value)?;
        crate::body_apply::output_bounds(&chunk)?;
        if chunk.entry != target.entry {
            return Err(ClientError::InvalidBody);
        }
        if let Some(token) = target.token {
            let result = self.bodies.apply_output(&token, chunk);
            return self.body_reply(serial, token, target.watch, result, effects);
        }
        effects.replies.push(ReadResult {
            id: Some(carlos_proto::commands::RequestId::Integer(serial)),
            reply: ReadReply::Output {
                session: target.session,
                chunk,
            },
        });
        Ok(())
    }

    fn body_reply(
        &mut self,
        serial: i64,
        token: crate::BodyToken,
        watch: bool,
        result: Result<bool, ClientError>,
        effects: &mut Effects,
    ) -> Result<(), ClientError> {
        match result {
            Ok(changed) => {
                if watch {
                    self.activate_watch(&token);
                }
                if changed {
                    effects.bodies.push(token);
                }
            }
            Err(ClientError::RetiredBody) => {}
            Err(error @ ClientError::Capacity(_)) => {
                if watch {
                    effects.bodies.push(token.clone());
                    self.failed_watch(&token);
                }
                effects.failures.push(crate::RequestFailure {
                    id: Some(carlos_proto::commands::RequestId::Integer(serial)),
                    error: RpcError {
                        code: -32012,
                        message: error.to_string(),
                    },
                });
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }

    pub(crate) fn failed(
        &mut self,
        pending: Option<(i64, Kind)>,
        error: &RpcError,
        effects: &mut Effects,
    ) {
        let Some((serial, kind)) = pending else {
            return;
        };
        match kind {
            Kind::GoalWatch(session) => self.goals.failed(&session, serial),
            Kind::Hello => {
                if let Some(connection) = self.current {
                    self.disconnect(connection);
                }
            }
            Kind::Mutation(operation) => {
                if self
                    .operations
                    .fail_local(&operation, &error.message, false)
                {
                    effects.operations.push(operation);
                }
            }
            Kind::Operation(operation) => {
                if error.code == -32006
                    && self.operations.fail_local(&operation, &error.message, true)
                {
                    effects.operations.push(operation);
                }
            }
            Kind::Source(target) | Kind::Output(target) if target.watch => {
                if let Some(token) = target.token {
                    effects.bodies.push(token.clone());
                    self.failed_watch(&token);
                }
            }
            Kind::Subscribe(session) => {
                if let Some(slot) = self
                    .sessions
                    .get_mut(&session)
                    .filter(|slot| slot.syncing == Some(serial))
                {
                    slot.syncing = None;
                    slot.refresh = false;
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn parse<T: DeserializeOwned>(value: Value) -> Result<T, ClientError> {
    serde_json::from_value(value)
        .map_err(|error| ClientError::InvalidPayload(error.to_string().chars().take(256).collect()))
}

fn same_session(
    expected: &carlos_proto::ids::SessionId,
    actual: &carlos_proto::ids::SessionId,
) -> Result<(), ClientError> {
    if expected == actual {
        Ok(())
    } else {
        Err(ClientError::ForeignSession)
    }
}
