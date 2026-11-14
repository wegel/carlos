//! Share compact native task facts while each device retains its own list and selected target.

use serde::{Deserialize, Serialize};

use crate::ids::{Sequence, SessionId, TaskRevision};

/// Preserve backend task counts independently of any client's bounded metadata page.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCounts {
    pub agents: u32,
    pub processes: u32,
    pub tasks: u32,
    pub active: u32,
}

/// A current native metadata header does not contain a selected row or private stop result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFacts {
    pub revision: TaskRevision,
    pub overview_revision: TaskRevision,
    pub known: bool,
    pub reading: bool,
    pub closed: bool,
    pub counts: TaskCounts,
    pub native_older_available: bool,
    pub read_error: Option<String>,
    pub notice: String,
}

/// A visible-list interest starts at one accepted session cursor, including cached native reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub session: SessionId,
    pub seq: Sequence,
    pub tasks: TaskFacts,
}
