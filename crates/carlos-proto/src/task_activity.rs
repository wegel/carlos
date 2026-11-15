//! Read one captured child's bounded activity without exporting native request parameters or JSON.

use serde::{Deserialize, Serialize};

use crate::ids::{Sequence, SessionId, TaskActivityId, TaskActivityItemId, TaskId, TaskRevision};

/// Native child reads retain at most the existing number of records in one interest.
pub const MAX_ACTIVITY_ITEMS: usize = 128;
/// One scalar keeps the legacy child panel's per-record display bound.
pub const MAX_ACTIVITY_TEXT: usize = 32 * 1024;
/// Encoded pages leave room inside the existing half-MiB owner reply reservation.
pub const MAX_ACTIVITY_PAGE_BYTES: usize = 384 * 1024;

/// One reader's native child, pending read and live tail remain distinct from the root transcript.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActivityHeader {
    pub session: SessionId,
    pub activity: TaskActivityId,
    pub task: TaskId,
    pub revision: TaskRevision,
    /// This cursor belongs only to this activity interest, never to the session's event stream.
    pub change: Sequence,
    pub items: u16,
    pub known: bool,
    pub reading: bool,
    pub has_older: bool,
    pub closed: bool,
    pub error: Option<String>,
    pub tail: String,
}

/// Labels describe native records; only this opaque id can continue an accepted display page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActivityRecord {
    pub id: TaskActivityItemId,
    pub turn: String,
    pub item: String,
    pub kind: String,
    pub text: String,
    pub complete: bool,
}

/// Oldest-first records feed the shared child panel, with byte bounds independent of native paging.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActivityPage {
    pub session: SessionId,
    pub activity: TaskActivityId,
    pub revision: TaskRevision,
    pub records: Vec<TaskActivityRecord>,
    pub has_more: bool,
}
