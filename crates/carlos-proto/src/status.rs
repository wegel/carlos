//! Small activity facts complement existing context, quota and accepted model choices.

use serde::{Deserialize, Serialize};

use crate::task_overview::{TaskLabel, TaskMainState};

/// A displayed transient label includes its ellipsis within this UTF-8 byte bound.
pub const MAX_STATUS_LABEL_BYTES: usize = 2048;

/// Native main activity remains separate from the lifetime of a submitted human turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MainActivity {
    pub state: TaskMainState,
    pub thinking_tokens: Option<u64>,
    pub transient: Option<TaskLabel>,
}

/// Preserve automatic-work guards without exposing queued human or automatic prompt text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RalphStatus {
    pub toggle_pending: bool,
    pub waiting_for_user: bool,
    pub continuation_pending: bool,
}

/// A client paints these facts and computes reset countdowns using its own clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusFacts {
    pub context_fresh: bool,
    pub activity: Option<MainActivity>,
    pub callbacks_pending: u32,
    pub tasks_uncertain: bool,
    pub ralph: RalphStatus,
}
