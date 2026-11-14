//! Read bounded native task metadata without exposing backend methods or mutable selections.

use serde::{Deserialize, Serialize};

use crate::bytes::{ByteLength, ByteOffset};
use crate::ids::{Sequence, SessionId, TaskId, TaskRevision, TaskRun};

/// Keep escaped task pages within the existing half-MiB read reservation.
pub const MAX_PAGE_TASKS: usize = 32;

/// Distinguish native child agents, background processes and Claude tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Agent,
    Process,
    Task,
}

/// A compact row preserves unknown native states without claiming that idle means complete.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub kind: TaskKind,
    pub label: String,
    pub label_complete: bool,
    pub state: String,
    pub run: Option<TaskRun>,
}

/// Each client keeps its own page and selected task; reads never change a shared selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPage {
    pub session: SessionId,
    pub seq: Sequence,
    pub revision: TaskRevision,
    pub known: bool,
    pub reading: bool,
    pub closed: bool,
    pub tasks: Vec<Task>,
    pub has_more: bool,
    pub native_older_available: bool,
    pub read_error: Option<String>,
}

/// Only these native scalar fields can be read; callers cannot select arbitrary JSON paths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskField {
    NativeId,
    Owner,
    LastTurn,
    ProcessItem,
    RunLabel,
    CanStop,
    NativeState,
    Label,
    Role,
    Parent,
    DirectInput,
    LastTurnState,
    Report,
    ReportState,
    Command,
    Directory,
    OsPid,
    CpuPercent,
    RssKib,
    Description,
    Summary,
    NativeType,
    ParentCaller,
    AgentType,
    OutputFile,
    Error,
    UsageTokens,
    UsageTools,
    UsageDurationMillis,
    Ambient,
    SkipTranscript,
}

/// A clipped field advertises its full byte length so a client can read the exact value in chunks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskProperty {
    pub field: TaskField,
    pub text: String,
    pub bytes: ByteLength,
    pub complete: bool,
}

/// Metadata previews share one revision with the task page and remain independent of task history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDetail {
    pub session: SessionId,
    pub revision: TaskRevision,
    pub task: Task,
    pub properties: Vec<TaskProperty>,
}

/// Exact field reads preserve Unicode and report a discarded prefix explicitly by byte offset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskText {
    pub session: SessionId,
    pub revision: TaskRevision,
    pub task: TaskId,
    pub field: TaskField,
    pub offset: ByteOffset,
    pub next_offset: ByteOffset,
    pub complete: bool,
    pub text: String,
}

/// A read identifies a current native run without claiming that a stop has been sent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRunSnapshot {
    pub session: SessionId,
    pub task: TaskId,
    pub read: Option<crate::ids::TaskReadId>,
    pub run: Option<TaskRun>,
    pub known: bool,
    pub native_state: Option<String>,
    pub reading: bool,
    pub closed: bool,
    pub error: Option<String>,
}
