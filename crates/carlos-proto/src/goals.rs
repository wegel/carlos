//! Share native root-goal facts and explicit edits without exposing native thread parameters.

use serde::{Deserialize, Serialize};

use crate::ids::{GoalRevision, Sequence};

/// Bound new objective source before building native parameters.
pub const MAX_OBJECTIVE_BYTES: usize = 32 * 1024;
/// Bound native goal objects, including escaped source and backend counters.
pub const MAX_GOAL_BYTES: usize = 128 * 1024;

/// Unknown goal state must not permit a competing automatic driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalKnowledge {
    Unknown,
    Known,
    Unavailable,
}

/// A native status explicitly chosen by a client; a missing field never defaults to active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GoalStatus {
    Active,
    Paused,
    Blocked,
    UsageLimited,
    BudgetLimited,
    Complete,
}

impl GoalStatus {
    /// Translate the typed choice to the native goal field inside the trusted adapter.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Blocked => "blocked",
            Self::UsageLimited => "usageLimited",
            Self::BudgetLimited => "budgetLimited",
            Self::Complete => "complete",
        }
    }
}

/// Accepted native values; an unrecognized future status stays visible without becoming writable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub objective: String,
    pub status: String,
    pub token_budget: Option<Sequence>,
    pub tokens_used: Sequence,
    pub time_used_seconds: Sequence,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Complete root-goal facts; mutation diagnostics belong to their private operation result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalFacts {
    pub revision: GoalRevision,
    pub knowledge: GoalKnowledge,
    pub goal: Option<Goal>,
    pub reading: bool,
    pub pending: bool,
    pub direct_input: Option<bool>,
    pub read_error: Option<String>,
    pub notice: String,
}

/// Omitted objective/budget retain their native values; clearing counters requires goal/clear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalChange {
    pub objective: Option<String>,
    pub token_budget: Option<Sequence>,
    pub status: GoalStatus,
}

/// A current read stamped in the same session stream; a late reply cannot undo newer goal facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalSnapshot {
    pub session: crate::ids::SessionId,
    pub seq: Sequence,
    pub goal: GoalFacts,
}
