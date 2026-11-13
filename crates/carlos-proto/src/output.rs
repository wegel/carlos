//! Bounded output reads and optional live subscriptions, separate from session ordering.

use serde::{Deserialize, Serialize};

use crate::ids::{EntryId, Sequence, SessionId};
use crate::session::OutputKind;

/// The current body generation, distinct from the session event cursor.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutputRevision(Sequence);

impl OutputRevision {
    /// Identify the initial body or a later non-append replacement.
    pub const fn new(value: u64) -> Self {
        Self(Sequence::new(value))
    }

    /// Read the generation without rounding it.
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

/// An entry-bound parsing position; clients echo it without interpreting its contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OutputCursor(String);

impl OutputCursor {
    /// Borrow a bounded opaque token for a subsequent read or subscription.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for OutputCursor {
    type Error = crate::ids::InvalidId;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > 256 || value.is_empty() || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(crate::ids::InvalidId);
        }
        Ok(Self(value))
    }
}

impl From<OutputCursor> for String {
    fn from(value: OutputCursor) -> Self {
        value.0
    }
}

/// A piece of a logical line, including any source newlines; joins preserve the body exactly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputSegment {
    pub kind: OutputKind,
    pub text: String,
}

/// A bounded read at a sealed session cursor; an empty chunk may still advance past controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputChunk {
    pub entry: EntryId,
    pub revision: OutputRevision,
    pub from: Option<OutputCursor>,
    pub next: OutputCursor,
    pub segments: Vec<OutputSegment>,
    pub has_more: bool,
    pub done: bool,
    pub seq: Sequence,
}

/// Live output uses its own resumable cursor so collapsed clients skip bulk data without seq gaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputUpdate {
    pub session: SessionId,
    pub chunk: OutputChunk,
}
