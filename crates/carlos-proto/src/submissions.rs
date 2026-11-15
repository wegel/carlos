//! Correlate submitted text without carrying a client's live editor state.

use serde::{Deserialize, Serialize};

use crate::ids::{ClientId, OperationId};

/// A command's origin; the daemon binds the client id to its authenticated connection.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ClientOperation {
    pub client: ClientId,
    pub id: OperationId,
}

/// Address one logical session with the caller's retained operation counter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionOperation {
    pub session: crate::ids::SessionId,
    pub operation: ClientOperation,
}

/// Accepted command text retained for asynchronous queue or fork recovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmittedText {
    pub operation: Option<ClientOperation>,
    pub text: String,
}

/// Why submitted text returned without becoming an accepted prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryKind {
    Prompt,
    Edit,
    Queued,
}

/// Facts about one submitted command; the receiving client chooses its own editor response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum SubmissionOutcome {
    PromptAccepted(Option<ClientOperation>),
    EditStarted(Option<ClientOperation>),
    EditRejected(Option<ClientOperation>),
    EditAccepted(Option<ClientOperation>),
    EditQueued(SubmittedText),
    Recovered {
        draft: SubmittedText,
        kind: RecoveryKind,
    },
}

/// One command's private, reconnectable result; accepted means native admission, not turn completion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationResult {
    pub operation: ClientOperation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<crate::ids::SessionId>,
    /// The owner adopted this operation's fork; a later send failure must not fork again.
    #[serde(default, skip_serializing_if = "is_false")]
    pub rewound: bool,
    pub state: OperationState,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// A command keeps submitted text recoverable when its outcome cannot be confirmed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OperationState {
    Pending,
    Queued,
    Accepted {
        entry: Option<crate::ids::EntryId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        notice: Option<String>,
    },
    Rejected {
        message: String,
        recovery: Option<SubmittedText>,
    },
    Uncertain {
        message: String,
        recovery: Option<SubmittedText>,
    },
}
