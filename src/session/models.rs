//! Source messages and conversation facts without terminal styles or layout.

use super::entry_id::EntryId;

/// The speaker role for a transcript message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
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
pub(crate) enum TurnStatus {
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
    pub(crate) fn is_terminal(self) -> bool {
        self != Self::InProgress
    }
}

/// The source text and plain facts for one transcript entry.
#[derive(Debug, Clone)]
pub(crate) struct Message {
    pub(crate) id: EntryId,
    pub(crate) tool_output: Option<ToolOutputDetails>,
    pub(crate) role: Role,
    pub(crate) text: String,
    pub(crate) kind: MessageKind,
    pub(crate) file_path: Option<String>,
}

/// Keep a tool's label and failure state alongside its original contents.
#[derive(Debug, Clone)]
pub(crate) struct ToolOutputDetails {
    pub(crate) generated: bool,
    pub(crate) summary: String,
    pub(crate) failed: bool,
}

/// The format of a stored transcript entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageKind {
    Plain,
    UserSteer,
    Diff,
}

/// The source diff for one file.
#[derive(Debug, Clone)]
pub(crate) struct DiffBlock {
    pub(crate) file_path: Option<String>,
    pub(crate) diff: String,
}

/// The backend facts shown for one saved conversation.
#[derive(Debug, Clone)]
pub(crate) struct ThreadSummary {
    pub(crate) id: String,
    pub(crate) name: Option<String>,
    pub(crate) preview: String,
    pub(crate) cwd: String,
    pub(crate) created_at: i64,
    pub(crate) updated_at: i64,
}

/// An entry whose id is assigned only when the transcript adopts it.
#[derive(Debug, Clone)]
pub(crate) struct EntryDraft {
    pub(crate) tool_output: Option<ToolOutputDetails>,
    pub(crate) role: Role,
    pub(crate) text: String,
    pub(crate) kind: MessageKind,
    pub(crate) file_path: Option<String>,
}

impl EntryDraft {
    /// Build a plain entry without allocating an id.
    pub(crate) fn plain(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            kind: MessageKind::Plain,
            file_path: None,
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
        }
    }
}
