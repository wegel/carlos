//! Display facts shared by terminal, phone and browser clients.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::{EntryId, Sequence, SessionId};

/// A backend chosen for a session, independent of enabled compatibility controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Codex,
    Claude,
    Grok,
}

/// The session's activity, with approval taking precedence over a running turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Running,
    Approval,
}

/// Token counts last reported by the backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Context {
    pub used: u64,
    pub max: u64,
}

/// An absolute reset time lets a client update its quota countdown locally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quota {
    pub remaining_pct: f64,
    pub resets_at: Option<i64>,
}

/// Session facts for headers and session lists; missing facts remain null.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub name: Option<String>,
    pub backend: Backend,
    pub model: Option<String>,
    pub host: String,
    pub cwd: Option<String>,
    pub phase: Phase,
    pub ralph: bool,
    pub turn_started_at: Option<i64>,
    pub context: Option<Context>,
    pub quota: Option<Quota>,
}

/// Source text classifications; clients decide how to wrap and style each kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    User,
    Reasoning,
    Commentary,
    Answer,
    Tool,
    Notice,
}

/// Tool execution facts; unknown is deliberately distinct from success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Running,
    Ok,
    Failed,
    Interrupted,
    Unknown,
}

/// The backend's tool family, without terminal glyphs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Command,
    Read,
    Search,
    Edit,
    Other,
}

/// How the core obtained the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LabelSource {
    Literal,
    Ai,
}

/// Compact tool facts; large output is read separately through entry/output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    pub kind: ToolKind,
    pub label: String,
    pub label_source: LabelSource,
    pub status: ToolStatus,
    pub summary: Option<String>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub has_output: bool,
}

/// An immutable row at a published cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: EntryId,
    pub kind: EntryKind,
    pub text: String,
    pub done: bool,
    pub tool: Option<Tool>,
    pub client_message_id: Option<String>,
}

/// Decisions are offered only when the backend supports the corresponding reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Once,
    Session,
    Deny,
    Cancel,
}

/// An approval card is separate from transcript history and has its own stable id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Approval {
    pub id: EntryId,
    pub kind: String,
    pub title: String,
    pub details: Vec<String>,
    pub choices: Vec<Decision>,
    pub input: Option<ApprovalInput>,
    pub submitted: bool,
    pub active: bool,
    #[serde(default)]
    pub default_to_no: bool,
}

/// Structured input selected explicitly from a backend request; no routing ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalInput {
    Fields { fields: Vec<InputField> },
    Form { schema: Value },
    Browser { url: String },
    Unsupported { reason: String },
}

/// A newest page and live cards at exactly the enclosed cursor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub info: SessionInfo,
    pub entries: Vec<Entry>,
    pub approvals: Vec<Approval>,
    pub seq: Sequence,
    pub has_older: bool,
}

/// An older page in chronological order at the supplied cursor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct History {
    pub entries: Vec<Entry>,
    pub has_older: bool,
    pub seq: Sequence,
}

/// A semantic output line; clients choose colors, fonts and horizontal scrolling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputLine {
    pub kind: OutputKind,
    pub text: String,
}

/// Line classifications require no terminal escape codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Plain,
    Addition,
    Deletion,
    Hunk,
    Context,
    Command,
    Success,
    Error,
}

/// A neutral question field; its key is local to this card, never a backend route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputField {
    pub key: String,
    pub title: String,
    pub description: String,
    pub secret: bool,
    pub multiple: bool,
    pub other: bool,
    pub notes_only: bool,
    pub required: bool,
    pub schema: Value,
    pub options: Vec<InputOption>,
}

/// A listed answer retains its typed value separately from the displayed label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputOption {
    pub value: Value,
    pub label: String,
    pub description: String,
}
