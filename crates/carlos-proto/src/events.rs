//! Numbered changes in the single ordered stream for a session.

use serde::{Deserialize, Serialize};

use crate::commands::RpcVersion;
use crate::ids::{EntryId, Sequence, SessionId};
use crate::output::{OutputRevision, OutputUpdate};
use crate::session::{
    Approval, Decision, Entry, EntryFormat, EntryKind, SessionInfo, Snapshot, Tool,
};

/// Facts that change without replacing or resending an entry's source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryFacts {
    pub kind: EntryKind,
    #[serde(default, skip_serializing_if = "EntryFormat::is_plain")]
    pub format: EntryFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    pub done: bool,
    pub tool: Option<Tool>,
}

/// A change after the preceding cursor; replacements are explicit, not nullable patches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    SettingsChanged {
        settings: Option<crate::settings::SettingsFacts>,
    },
    TasksChanged {
        tasks: Option<crate::task_facts::TaskFacts>,
    },
    QueueChanged {
        queue: Option<crate::queue::QueueFacts>,
    },
    GoalChanged {
        goal: Option<crate::goals::GoalFacts>,
    },
    EntryAdded {
        entry: Entry,
    },
    TextAppended {
        entry: EntryId,
        text: String,
    },
    EntryUpdated {
        entry: Entry,
    },
    EntryFactsChanged {
        entry: EntryId,
        facts: EntryFacts,
    },
    OutputChanged {
        entry: EntryId,
        revision: OutputRevision,
        has_output: bool,
    },
    SourceChanged {
        entry: EntryId,
        source: crate::source::Source,
    },
    ApprovalRequested {
        approval: Approval,
        before: Option<EntryId>,
    },
    ApprovalResolved {
        approval: EntryId,
        decision: Option<Decision>,
    },
    InfoChanged {
        info: SessionInfo,
    },
    Reset {
        snapshot: Box<Snapshot>,
    },
}

/// An event's cursor advances only within its own logical session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub session: SessionId,
    pub seq: Sequence,
    pub event: Event,
}

/// Daemon notifications use JSON-RPC 2.0 without a request id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Notification {
    #[serde(rename = "entry/source")]
    Source(crate::source::SourceUpdate),
    #[serde(rename = "entry/source/reset")]
    SourceReset {
        session: SessionId,
        entry: EntryId,
        message: String,
    },
    #[serde(rename = "session/syncFailed")]
    SyncFailed {
        session: SessionId,
        error: crate::commands::RpcError,
    },
    #[serde(rename = "session/snapshot")]
    Snapshot { snapshot: Box<Snapshot> },
    #[serde(rename = "entry/output/reset")]
    OutputReset {
        session: SessionId,
        entry: EntryId,
        message: String,
    },
    #[serde(rename = "operation/updated")]
    Operation(crate::submissions::OperationResult),
    #[serde(rename = "session/event")]
    SessionEvent(SessionEvent),
    #[serde(rename = "sessions/changed")]
    SessionsChanged(SessionChange),
    #[serde(rename = "entry/output")]
    Output(OutputUpdate),
}

/// Session list changes either supply the complete current facts or remove an id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionChange {
    Updated { info: SessionInfo },
    Removed { session: SessionId },
}

/// A complete JSON-RPC notification envelope, with no correlation id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationMessage {
    pub jsonrpc: RpcVersion,
    #[serde(flatten)]
    pub notification: Notification,
}
