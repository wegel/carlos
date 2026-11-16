//! Source messages and conversation facts without terminal styles or layout.

use carlos_proto::session::{ToolKind, ToolStatus};

use super::entry_id::EntryId;

/// The speaker role for a transcript message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Commentary,
    Reasoning,
    ToolCall,
    ToolOutput,
    System,
}

/// Final or current state reported for one app-server turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnStatus {
    InProgress,
    Completed,
    Interrupted,
    Failed,
}

impl TurnStatus {
    /// Parse the app-server wire value for a turn status.
    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        match value {
            "inProgress" => Some(Self::InProgress),
            "completed" => Some(Self::Completed),
            "interrupted" => Some(Self::Interrupted),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    /// Return whether app-server has ended the turn.
    pub fn is_terminal(self) -> bool {
        self != Self::InProgress
    }
}

/// The source text and plain facts for one transcript entry.
#[derive(Debug, Clone)]
pub struct Message {
    pub id: EntryId,
    pub tool_facts: Option<Box<ToolFacts>>,
    pub tool_output: Option<ToolOutputDetails>,
    pub role: Role,
    pub text: String,
    pub kind: MessageKind,
    pub file_path: Option<String>,
}

/// Keep a tool's label and failure state alongside its original contents.
#[derive(Debug, Clone)]
pub struct ToolOutputDetails {
    pub generated: bool,
    pub summary: String,
    pub failed: bool,
}

/// Native tool facts cached at item updates, independently of a generated label.
#[derive(Debug, Clone)]
pub struct ToolFacts {
    pub kind: ToolKind,
    pub status: ToolStatus,
    pub summary: Option<String>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
}

/// The format of a stored transcript entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Plain,
    UserSteer,
    Diff,
}

/// The source diff for one file.
#[derive(Debug, Clone)]
pub struct DiffBlock {
    pub file_path: Option<String>,
    pub diff: String,
}

/// The backend facts shown for one saved conversation.
#[derive(Debug, Clone)]
pub struct ThreadSummary {
    pub id: String,
    pub name: Option<String>,
    pub preview: String,
    pub cwd: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// An entry whose id is assigned only when the transcript adopts it.
#[derive(Debug, Clone)]
pub struct EntryDraft {
    pub tool_facts: Option<Box<ToolFacts>>,
    pub tool_output: Option<ToolOutputDetails>,
    pub role: Role,
    pub text: String,
    pub kind: MessageKind,
    pub file_path: Option<String>,
}

impl EntryDraft {
    /// Build a plain entry without allocating an id.
    pub fn plain(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            kind: MessageKind::Plain,
            file_path: None,
            tool_facts: None,
            tool_output: None,
        }
    }
}

impl From<Message> for EntryDraft {
    fn from(message: Message) -> Self {
        Self {
            role: message.role,
            text: message.text,
            kind: message.kind,
            file_path: message.file_path,
            tool_output: message.tool_output,
            tool_facts: message.tool_facts,
        }
    }
}
