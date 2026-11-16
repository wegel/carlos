//! Read bounded native queue views and change only explicitly selected, current entries.

use serde::{Deserialize, Serialize};

use crate::bytes::{ByteLength, ByteOffset};
use crate::ids::{QueueEntryId, QueueRevision, Sequence, SessionId};

/// Match the terminal's ordinary submitted-text bound without limiting persisted native inputs.
pub const MAX_TEXT_BYTES: usize = 512 * 1024;
/// Preserve the existing complete native queue allowance without growing cursor storage indefinitely.
pub const MAX_ENTRIES: usize = 4096;
/// Keep even escaped previews below a half-MiB reply reservation.
pub const MAX_PAGE_ENTRIES: usize = 32;
/// Keep the native identifier shown by the old queue header within a small display-only label.
pub const MAX_DISPLAY_ID_BYTES: usize = 256;

/// Shared queue facts contain no selected entry, editor or mutation failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueFacts {
    pub revision: QueueRevision,
    pub known: bool,
    pub reading: bool,
    pub pending: bool,
    pub count: u32,
    pub read_error: Option<String>,
    pub notice: String,
}

/// The daemon retains attachments and rich spans; clients can edit only complete plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueEntry {
    pub id: QueueEntryId,
    /// Read-only native label; commands must use the opaque id above.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_id: Option<String>,
    pub client_message: String,
    pub preview: String,
    pub preview_complete: bool,
    pub text_bytes: Option<ByteLength>,
}

/// Each client pages independently through one revision without selecting native read targets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuePage {
    pub session: SessionId,
    pub seq: Sequence,
    pub queue: QueueFacts,
    pub entries: Vec<QueueEntry>,
    pub has_more: bool,
}

/// Exact editable text arrives in UTF-8-safe chunks instead of enlarging queue pages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueueText {
    pub session: SessionId,
    pub revision: QueueRevision,
    pub entry: QueueEntryId,
    pub offset: ByteOffset,
    pub next_offset: ByteOffset,
    pub complete: bool,
    pub text: String,
}

/// An explicit move swaps adjacent entries in the accepted native queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueDirection {
    Up,
    Down,
}

/// Clients supply plain values, never native method names, input JSON or backend ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueueChange {
    Add {
        text: String,
    },
    Edit {
        entry: QueueEntryId,
        text: String,
    },
    Delete {
        entry: QueueEntryId,
    },
    Start {
        entry: QueueEntryId,
    },
    Move {
        entry: QueueEntryId,
        direction: QueueDirection,
    },
}

impl QueueChange {
    /// Borrow recoverable submitted text without cloning a queue editor.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Add { text } | Self::Edit { text, .. } => Some(text),
            _ => None,
        }
    }
}
