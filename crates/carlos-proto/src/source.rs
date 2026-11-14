//! Exact original entry text, transferred separately from compact facts and normalized output.

use serde::{Deserialize, Serialize};

use crate::bytes::{ByteLength, ByteOffset};
use crate::ids::{EntryId, InvalidId, Sequence, SessionId};
use crate::output::OutputRevision;

/// Original source larger than this travels through bounded source reads instead of row facts.
pub const INLINE_SOURCE_BYTES: usize = 4096;

/// A source-bound resume token, deliberately distinct from a normalized output cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceCursor(String);

impl SourceCursor {
    /// Borrow the bounded opaque token for a later exact-source read.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SourceCursor {
    type Error = InvalidId;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() > 256 || value.is_empty() || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(InvalidId);
        }
        Ok(Self(value))
    }
}

impl From<SourceCursor> for String {
    fn from(value: SourceCursor) -> Self {
        value.0
    }
}

/// A separately readable original body; bytes include terminal controls and UTF-8 text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub revision: OutputRevision,
    pub bytes: ByteLength,
}

/// A bounded exact UTF-8 slice; concatenate text in cursor order before interpreting controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceChunk {
    pub entry: EntryId,
    pub offset: ByteOffset,
    pub source: Source,
    pub from: Option<SourceCursor>,
    pub next: SourceCursor,
    pub text: String,
    pub has_more: bool,
    pub done: bool,
    pub seq: Sequence,
}

/// Original-source watches have independent cursors and never put bulk text in session events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceUpdate {
    pub session: SessionId,
    pub chunk: SourceChunk,
}
