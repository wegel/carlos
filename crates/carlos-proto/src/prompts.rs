//! Select immutable prompt records without exposing native turns or storing a shared cursor.

use serde::{Deserialize, Serialize};

use crate::bytes::{ByteLength, ByteOffset};
use crate::ids::{EntryId, SessionId};

/// Unknown-source Claude records require an explicit choice separate from human history.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptSource {
    Human,
    Unknown,
}

/// Move relative to a stable record rather than trusting an old numeric position.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptDirection {
    Latest,
    Older,
    Newer,
}

/// A pending prompt can have an owner record before it has a transcript entry.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptRecord {
    pub id: EntryId,
    pub entry: Option<EntryId>,
    pub source: PromptSource,
    pub bytes: ByteLength,
}

/// One reader's selected record; another device never changes its cursor.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptSelection {
    pub session: SessionId,
    pub source: PromptSource,
    pub record: Option<PromptRecord>,
    pub has_older: bool,
    pub has_newer: bool,
}

/// Exact original prompt text, split only at UTF-8 boundaries and bounded to 64 KiB.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PromptText {
    pub session: SessionId,
    pub prompt: EntryId,
    pub offset: ByteOffset,
    pub total: ByteLength,
    pub text: String,
    pub next: Option<ByteOffset>,
}
