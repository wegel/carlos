//! Report protocol and bounded-cache failures without choosing a platform's recovery UI.

use std::fmt;

use carlos_proto::ids::{EntryId, Sequence, SessionId};

/// A client must refresh stale facts instead of guessing how an invalid delta should apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientError {
    FrameTooLarge,
    InvalidEnvelope,
    InvalidConnection,
    InvalidPayload(String),
    ForeignSession,
    DuplicateEntry(EntryId),
    SequenceGap {
        session: SessionId,
        expected: Sequence,
        received: Sequence,
    },
    StaleHistory,
    StaleGoal,
    Capacity(&'static str),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FrameTooLarge => f.write_str("server frame exceeds eight MiB"),
            Self::InvalidEnvelope => f.write_str("invalid JSON-RPC server envelope"),
            Self::InvalidConnection => {
                f.write_str("frame or command does not belong to the current greeted connection")
            }
            Self::InvalidPayload(message) => write!(f, "invalid server payload: {message}"),
            Self::ForeignSession => f.write_str("facts belong to another session"),
            Self::DuplicateEntry(entry) => write!(f, "duplicate entry address: {entry}"),
            Self::SequenceGap {
                expected, received, ..
            } => write!(
                f,
                "session event gap: expected {}, received {}",
                expected.get(),
                received.get()
            ),
            Self::StaleHistory => {
                f.write_str("history was superseded; read again from the current first entry")
            }
            Self::StaleGoal => f.write_str("goal read was superseded by newer session facts"),
            Self::Capacity(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ClientError {}
