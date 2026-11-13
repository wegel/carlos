//! Typed JSON-RPC commands; transports and backend dispatch live outside this crate.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::SessionEvent;
use crate::ids::{EntryId, Sequence, SessionId};
use crate::session::{Backend, Decision, OutputLine, SessionInfo, Snapshot};
use crate::PROTOCOL_VERSION;

/// Only JSON-RPC version 2.0 is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

/// String ids avoid numeric narrowing; numeric ids are restricted to signed 64-bit integers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Integer(i64),
}

/// A client request, including its response correlation id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub jsonrpc: RpcVersion,
    pub id: RequestId,
    #[serde(flatten)]
    pub command: Command,
}

/// Commands use opaque addresses and never accept native backend request ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Command {
    #[serde(rename = "hello")]
    Hello {
        protocol: u32,
        client: String,
        token: String,
    },
    #[serde(rename = "sessions/watch")]
    Watch {},
    #[serde(rename = "session/create")]
    Create {
        backend: Backend,
        cwd: String,
        name: Option<String>,
    },
    #[serde(rename = "session/subscribe")]
    Subscribe {
        session: SessionId,
        since: Option<Sequence>,
    },
    #[serde(rename = "session/unsubscribe")]
    Unsubscribe { session: SessionId },
    #[serde(rename = "session/history")]
    History {
        session: SessionId,
        before: EntryId,
        limit: u16,
    },
    #[serde(rename = "session/send")]
    Send {
        session: SessionId,
        text: String,
        client_message_id: String,
    },
    #[serde(rename = "session/interrupt")]
    Interrupt { session: SessionId },
    #[serde(rename = "session/approve")]
    Approve {
        session: SessionId,
        approval: EntryId,
        answer: ApprovalAnswer,
    },
    #[serde(rename = "session/setRalph")]
    SetRalph { session: SessionId, on: bool },
    #[serde(rename = "entry/output")]
    Output { session: SessionId, entry: EntryId },
}

/// Input answers are validated against the live card before any backend reply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ApprovalAnswer {
    Decision { decision: Decision },
    Input { value: Value },
}

/// The server's greeting after version and token checks succeed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloResult {
    pub protocol: u32,
    pub host: String,
    pub daemon_version: String,
}

/// An explicit response result, without ambiguous result/error objects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Success<T> {
    pub jsonrpc: RpcVersion,
    pub id: RequestId,
    pub result: T,
}

/// A rejected command; an invalid envelope may have a null correlation id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    pub jsonrpc: RpcVersion,
    pub id: Option<RequestId>,
    pub error: RpcError,
}

/// Error codes follow JSON-RPC for standard errors and use -320xx for Carlos failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
}

/// Current sessions returned by sessions/watch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sessions {
    pub sessions: Vec<SessionInfo>,
}

/// Reject a protocol mismatch before a subscription or command is accepted.
pub fn check_protocol(version: u32) -> Result<(), RpcError> {
    if version == PROTOCOL_VERSION {
        return Ok(());
    }
    Err(RpcError {
        code: -32001,
        message: format!("unsupported protocol {version}; expected {PROTOCOL_VERSION}"),
    })
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

/// A sent message's stable address matches the echoed client_message_id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendResult {
    pub entry: EntryId,
}

/// Expanded output is tied to a published cursor so stale fetches can be discarded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputResult {
    pub lines: Vec<OutputLine>,
    pub seq: Sequence,
}
