//! Numbered changes in the single ordered stream for a session.

use serde::{Deserialize, Serialize};

use crate::commands::RpcVersion;
use crate::ids::{EntryId, Sequence, SessionId};
use crate::session::{Approval, Entry, EntryKind, SessionInfo, Snapshot, Tool};

/// Facts that change without replacing or resending an entry's source text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryFacts {
    pub kind: EntryKind,
    pub done: bool,
    pub tool: Option<Tool>,
}

/// A change after the preceding cursor; replacements are explicit, not nullable patches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    EntryAdded { entry: Entry },
    TextAppended { entry: EntryId, text: String },
    EntryUpdated { entry: Entry },
    EntryFactsChanged { entry: EntryId, facts: EntryFacts },
    ApprovalRequested { approval: Approval },
    ApprovalResolved { approval: EntryId },
    InfoChanged { info: SessionInfo },
    Reset { snapshot: Snapshot },
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
    #[serde(rename = "session/event")]
    SessionEvent(SessionEvent),
    #[serde(rename = "sessions/changed")]
    SessionsChanged(SessionChange),
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
