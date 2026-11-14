//! Read existing task-panel activity, quota windows and accepted prompt states in bounded pages.

use serde::{Deserialize, Serialize};

use crate::bytes::{ByteLength, ByteOffset};
use crate::ids::{Sequence, SessionId, TaskOverviewId, TaskRevision};

/// A named display label keeps exact text available without returning an unbounded metadata field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLabel {
    pub text: String,
    pub bytes: ByteLength,
    pub complete: bool,
}

/// Main-process activity remains distinct from a background task's actual lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskMainState {
    Idle,
    Running,
    RequiresAction,
}

/// Only existing panel facts are shared; accepted prompt text and private answers stay absent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskOverviewData {
    Main {
        state: TaskMainState,
        thinking_tokens: Option<u64>,
        transient: Option<TaskLabel>,
    },
    Quota {
        label: TaskLabel,
        used_pct: f64,
        resets_at: i64,
    },
    Prompt {
        label: TaskLabel,
        consumed: bool,
        steer: bool,
    },
    ProcessError {
        label: TaskLabel,
        message: TaskLabel,
    },
}

/// An opaque row address permits a checked continuation without exposing a backend route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOverviewEntry {
    pub id: TaskOverviewId,
    pub data: TaskOverviewData,
}

/// Overview rows share the task family's existing 32-row wire bound.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOverviewPage {
    pub session: SessionId,
    pub seq: Sequence,
    pub revision: TaskRevision,
    pub entries: Vec<TaskOverviewEntry>,
    pub has_more: bool,
}

/// Read only the displayed label or diagnostic of an explicitly addressed overview row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskOverviewField {
    Label,
    Message,
}

/// Exact named text reads preserve Unicode independently of preview and terminal width.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskOverviewText {
    pub session: SessionId,
    pub revision: TaskRevision,
    pub entry: TaskOverviewId,
    pub field: TaskOverviewField,
    pub offset: ByteOffset,
    pub next_offset: ByteOffset,
    pub complete: bool,
    pub text: String,
}
