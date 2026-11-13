//! Correlate submitted text without carrying a client's live editor state.

use serde::{Deserialize, Serialize};

use crate::ids::{ClientId, OperationId};

/// A command's origin; the daemon binds the client id to its authenticated connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientOperation {
    pub client: ClientId,
    pub id: OperationId,
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
