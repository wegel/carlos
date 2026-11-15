//! Preserve typed command answers and transport replies independently of the command method list.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::SessionEvent;
use crate::ids::{ClientId, DaemonId, OperationId, ResumeToken, Sequence};
use crate::session::{Decision, SessionInfo, Snapshot};

/// Explicitly steer running work or queue text for a later turn.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SendPolicy {
    #[default]
    Steer,
    Queue,
}

/// Input answers are validated against the live card before any backend reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalAnswer {
    Decision { decision: Decision },
    Input { value: Value },
}

/// Structured question answers retain card-local keys and exact option values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldAnswers {
    pub fields: Vec<FieldAnswer>,
}

/// One field contains offered values and optional free text, never a backend request id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldAnswer {
    pub key: String,
    pub values: Vec<Value>,
    pub text: Option<String>,
}

/// The server's greeting after version and token checks succeed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloResult {
    pub protocol: u32,
    pub host: String,
    pub daemon_version: String,
    pub instance: DaemonId,
    pub client: ClientId,
    pub resumed: bool,
    pub next_operation: Option<OperationId>,
    pub resume_token: ResumeToken,
}

/// Current sessions returned by sessions/watch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sessions {
    pub sessions: Vec<SessionInfo>,
}

/// Subscription replies establish one cursor after either replay or replacement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubscribeResult {
    Replay {
        events: Vec<SessionEvent>,
        seq: Sequence,
    },
    Snapshot {
        snapshot: Box<Snapshot>,
    },
}
