//! Identify one captured native task-list window without exposing its backend cursor or rows.

use serde::{Deserialize, Serialize};

use crate::ids::{Sequence, SessionId, TaskRevision, TaskWindowId};

/// Bound all retained recent and older native task metadata to the existing shared allowance.
pub const MAX_NATIVE_TASK_BYTES: usize = 8 * 1024 * 1024;
/// Bound independently captured native windows rather than multiplying the per-window allowance.
pub const MAX_TASK_WINDOWS: usize = 8;

/// A private read context remains separate from the latest session task facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWindow {
    pub session: SessionId,
    pub window: TaskWindowId,
    pub seq: Sequence,
    pub revision: TaskRevision,
    pub rows: u32,
    pub known: bool,
    pub reading: bool,
    pub older_available: bool,
    pub read_error: Option<String>,
}
