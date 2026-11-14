//! Share bounded watch scheduling while keeping exact-source and normalized cursors independent.

use carlos_core::engine::{EngineCommand, EngineReply};
use carlos_proto::commands::{Command, RequestId, RpcError};
use carlos_proto::events::Notification;
use carlos_proto::ids::{EntryId, SessionId};
use carlos_proto::output::{OutputChunk, OutputCursor, OutputUpdate};
use carlos_proto::source::{SourceChunk, SourceCursor, SourceUpdate};
use serde::Serialize;

use crate::transport::outbox::Kind;
use crate::transport::wire::error;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Format {
    Normalized,
    Source,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Cursor {
    Normalized(Option<OutputCursor>),
    Source(Option<SourceCursor>),
}

#[derive(Serialize)]
#[serde(untagged)]
pub(super) enum Chunk {
    Normalized(OutputChunk),
    Source(SourceChunk),
}

impl Format {
    pub fn queue_kind(self, session: &SessionId, entry: &EntryId) -> Kind {
        match self {
            Self::Normalized => Kind::Output(session.clone(), entry.clone()),
            Self::Source => Kind::Source(session.clone(), entry.clone()),
        }
    }

    pub fn reset(self, session: SessionId, entry: EntryId, message: String) -> Notification {
        match self {
            Self::Normalized => Notification::OutputReset {
                session,
                entry,
                message,
            },
            Self::Source => Notification::SourceReset {
                session,
                entry,
                message,
            },
        }
    }
}

impl Cursor {
    pub fn format(&self) -> Format {
        match self {
            Self::Normalized(_) => Format::Normalized,
            Self::Source(_) => Format::Source,
        }
    }

    pub fn request(self, entry: EntryId) -> EngineCommand {
        match self {
            Self::Normalized(cursor) => EngineCommand::Output {
                entry,
                cursor,
                limit: 65536,
            },
            Self::Source(cursor) => EngineCommand::Source {
                entry,
                cursor,
                limit: 65536,
            },
        }
    }
}

impl Chunk {
    pub fn read(reply: EngineReply, format: Format) -> Result<Self, RpcError> {
        match (reply, format) {
            (EngineReply::Output(chunk), Format::Normalized) => Ok(Self::Normalized(chunk)),
            (EngineReply::Source(chunk), Format::Source) => Ok(Self::Source(chunk)),
            (EngineReply::Failure(reason), _) => Err(reason),
            _ => Err(error(-32603, "unexpected body reply")),
        }
    }

    pub fn cursor(&self) -> Cursor {
        match self {
            Self::Normalized(chunk) => Cursor::Normalized(Some(chunk.next.clone())),
            Self::Source(chunk) => Cursor::Source(Some(chunk.next.clone())),
        }
    }

    pub fn done(&self) -> bool {
        match self {
            Self::Normalized(chunk) => chunk.done,
            Self::Source(chunk) => chunk.done,
        }
    }

    pub fn has_more(&self) -> bool {
        match self {
            Self::Normalized(chunk) => chunk.has_more,
            Self::Source(chunk) => chunk.has_more,
        }
    }

    pub fn notification(&self, session: &SessionId) -> Notification {
        match self {
            Self::Normalized(chunk) => Notification::Output(OutputUpdate {
                session: session.clone(),
                chunk: chunk.clone(),
            }),
            Self::Source(chunk) => Notification::Source(SourceUpdate {
                session: session.clone(),
                chunk: chunk.clone(),
            }),
        }
    }
}

pub(super) type WatchKey = (SessionId, EntryId, Format);

impl super::Manager {
    pub(super) fn body_command(&mut self, peer: u64, id: RequestId, command: Command) {
        match command {
            Command::Output {
                session,
                entry,
                cursor,
                limit,
            } => self.rpc_read(
                peer,
                id,
                session,
                EngineCommand::Output {
                    entry,
                    cursor,
                    limit: limit.unwrap_or(65536).min(65536) as usize,
                },
            ),
            Command::OutputSubscribe {
                session,
                entry,
                cursor,
            } => self.watch_output(peer, id, session, entry, Cursor::Normalized(cursor)),
            Command::OutputUnsubscribe { session, entry } => {
                self.unwatch_body(peer, id, session, entry, Format::Normalized)
            }
            Command::Source {
                session,
                entry,
                cursor,
                limit,
            } => self.rpc_read(
                peer,
                id,
                session,
                EngineCommand::Source {
                    entry,
                    cursor,
                    limit: limit.unwrap_or(65536).min(65536) as usize,
                },
            ),
            Command::SourceSubscribe {
                session,
                entry,
                cursor,
            } => self.watch_output(peer, id, session, entry, Cursor::Source(cursor)),
            Command::SourceUnsubscribe { session, entry } => {
                self.unwatch_body(peer, id, session, entry, Format::Source)
            }
            _ => unreachable!("typed body route"),
        }
    }
}
