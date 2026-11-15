//! Typed JSON-RPC commands; transports and backend dispatch live outside this crate.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::events::SessionEvent;
use crate::ids::{
    ClientId, DaemonId, EntryId, OperationId, ResumeToken, Sequence, SessionId, TurnId,
};
use crate::output::OutputCursor;
pub use crate::rpc::{check_protocol, Failure, Request, RequestId, RpcError, RpcVersion, Success};
use crate::session::{Backend, Decision, SessionInfo, Snapshot};
use crate::submissions::ClientOperation;

/// Commands use opaque addresses and never accept native backend request ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Command {
    #[serde(rename = "task/subscribe")]
    TaskSubscribe { session: SessionId },
    #[serde(rename = "task/unsubscribe")]
    TaskUnsubscribe { session: SessionId },
    #[serde(rename = "task/overview")]
    TaskOverview {
        session: SessionId,
        expected: Option<crate::ids::TaskRevision>,
        after: Option<crate::ids::TaskOverviewId>,
        limit: Option<u16>,
    },
    #[serde(rename = "task/overview/text")]
    TaskOverviewText {
        session: SessionId,
        expected: crate::ids::TaskRevision,
        entry: crate::ids::TaskOverviewId,
        field: crate::task_overview::TaskOverviewField,
        offset: crate::bytes::ByteOffset,
        limit: Option<u32>,
    },
    #[serde(rename = "task/stop")]
    TaskStop {
        session: SessionId,
        operation: ClientOperation,
        task: crate::ids::TaskId,
        expected_run: crate::ids::TaskRun,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<crate::ids::TaskWindowId>,
    },
    #[serde(rename = "task/run")]
    TaskRun {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<crate::ids::TaskWindowId>,
        task: crate::ids::TaskId,
        read: Option<crate::ids::TaskReadId>,
        #[serde(default)]
        refresh: bool,
    },
    #[serde(rename = "task/list")]
    TaskList {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<crate::ids::TaskWindowId>,
        expected: Option<crate::ids::TaskRevision>,
        after: Option<crate::ids::TaskId>,
        limit: Option<u16>,
        #[serde(default)]
        refresh: bool,
    },
    #[serde(rename = "task/detail")]
    TaskDetail {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<crate::ids::TaskWindowId>,
        expected: crate::ids::TaskRevision,
        task: crate::ids::TaskId,
    },
    #[serde(rename = "task/text")]
    TaskText {
        session: SessionId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<crate::ids::TaskWindowId>,
        expected: crate::ids::TaskRevision,
        task: crate::ids::TaskId,
        field: crate::tasks::TaskField,
        offset: crate::bytes::ByteOffset,
        limit: Option<u32>,
    },
    #[serde(rename = "task/window/open")]
    TaskWindowOpen {
        session: SessionId,
        expected: crate::ids::TaskRevision,
        after: Option<crate::ids::TaskWindowId>,
    },
    #[serde(rename = "task/window/read")]
    TaskWindowRead {
        session: SessionId,
        window: crate::ids::TaskWindowId,
    },
    #[serde(rename = "task/window/close")]
    TaskWindowClose {
        session: SessionId,
        window: Option<crate::ids::TaskWindowId>,
    },
    #[serde(rename = "queue/read")]
    QueueRead {
        session: SessionId,
        expected: Option<crate::ids::QueueRevision>,
        after: Option<crate::ids::QueueEntryId>,
        limit: Option<u16>,
        #[serde(default)]
        refresh: bool,
    },
    #[serde(rename = "queue/text")]
    QueueText {
        session: SessionId,
        expected: crate::ids::QueueRevision,
        entry: crate::ids::QueueEntryId,
        offset: crate::bytes::ByteOffset,
        limit: Option<u32>,
    },
    #[serde(rename = "queue/change")]
    QueueChange {
        session: SessionId,
        operation: ClientOperation,
        expected: crate::ids::QueueRevision,
        change: crate::queue::QueueChange,
    },
    #[serde(rename = "goal/subscribe")]
    GoalSubscribe { session: SessionId },
    #[serde(rename = "goal/unsubscribe")]
    GoalUnsubscribe { session: SessionId },
    #[serde(rename = "goal/read")]
    GoalRead {
        session: SessionId,
        #[serde(default)]
        refresh: bool,
    },
    #[serde(rename = "goal/set")]
    GoalSet {
        session: SessionId,
        operation: ClientOperation,
        expected: crate::ids::GoalRevision,
        change: crate::goals::GoalChange,
    },
    #[serde(rename = "goal/clear")]
    GoalClear {
        session: SessionId,
        operation: ClientOperation,
        expected: crate::ids::GoalRevision,
    },
    #[serde(rename = "hello")]
    Hello {
        protocol: u32,
        client: String,
        #[serde(default)]
        token: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume: Option<ClientId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        resume_token: Option<ResumeToken>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        instance: Option<DaemonId>,
    },
    #[serde(rename = "sessions/watch")]
    Watch {},
    #[serde(rename = "session/create")]
    Create {
        operation: ClientOperation,
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
        operation: ClientOperation,
        #[serde(default)]
        policy: SendPolicy,
    },
    #[serde(rename = "session/rename")]
    Rename {
        session: SessionId,
        operation: ClientOperation,
        name: String,
    },
    #[serde(rename = "session/rewind")]
    Rewind {
        session: SessionId,
        operation: ClientOperation,
        entry: EntryId,
        text: String,
    },
    #[serde(rename = "session/interrupt")]
    Interrupt {
        session: SessionId,
        operation: ClientOperation,
        turn: TurnId,
    },
    #[serde(rename = "session/approve")]
    Approve {
        session: SessionId,
        approval: EntryId,
        operation: ClientOperation,
        answer: ApprovalAnswer,
    },
    #[serde(rename = "session/setRalph")]
    SetRalph {
        session: SessionId,
        operation: ClientOperation,
        on: bool,
    },
    #[serde(rename = "operation/read")]
    Operation { operation: ClientOperation },
    #[serde(rename = "entry/output")]
    Output {
        session: SessionId,
        entry: EntryId,
        cursor: Option<OutputCursor>,
        limit: Option<u32>,
    },
    #[serde(rename = "entry/output/subscribe")]
    OutputSubscribe {
        session: SessionId,
        entry: EntryId,
        cursor: Option<OutputCursor>,
    },
    #[serde(rename = "entry/output/unsubscribe")]
    OutputUnsubscribe { session: SessionId, entry: EntryId },
    #[serde(rename = "entry/source")]
    Source {
        session: SessionId,
        entry: EntryId,
        cursor: Option<crate::source::SourceCursor>,
        limit: Option<u32>,
    },
    #[serde(rename = "entry/source/seek")]
    SourceSeek {
        session: SessionId,
        entry: EntryId,
        revision: crate::output::OutputRevision,
        offset: crate::bytes::ByteOffset,
        limit: Option<u32>,
    },
    #[serde(rename = "entry/source/subscribe")]
    SourceSubscribe {
        session: SessionId,
        entry: EntryId,
        cursor: Option<crate::source::SourceCursor>,
    },
    #[serde(rename = "entry/source/unsubscribe")]
    SourceUnsubscribe { session: SessionId, entry: EntryId },
}

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
