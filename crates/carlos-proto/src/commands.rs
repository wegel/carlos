//! Typed JSON-RPC commands; transports and backend dispatch live outside this crate.

pub use crate::command_types::{
    ApprovalAnswer, FieldAnswer, FieldAnswers, HelloResult, SendPolicy, Sessions, SubscribeResult,
};
use crate::ids::{ClientId, DaemonId, EntryId, ResumeToken, Sequence, SessionId, TurnId};
use crate::output::OutputCursor;
pub use crate::rpc::{check_protocol, Failure, Request, RequestId, RpcError, RpcVersion, Success};
use crate::session::Backend;
use crate::submissions::ClientOperation;
use serde::{Deserialize, Serialize};

/// Commands use opaque addresses and never accept native backend request ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum Command {
    #[serde(rename = "saved/open")]
    SavedOpen {
        backend: Backend,
        cwd: String,
        codex_socket: Option<String>,
        purpose: crate::startup::SavedPurpose,
    },
    #[serde(rename = "saved/page")]
    SavedPage {
        scope: crate::ids::SavedScopeId,
        direction: crate::startup::SavedDirection,
    },
    #[serde(rename = "saved/close")]
    SavedClose {
        scope: Option<crate::ids::SavedScopeId>,
    },
    #[serde(rename = "session/open")]
    Open {
        operation: ClientOperation,
        backend: Backend,
        cwd: String,
        target: crate::startup::OpenTarget,
        #[serde(default)]
        launch: crate::startup::NativeLaunchOptions,
        #[serde(default)]
        tool_labels: Option<bool>,
    },
    #[serde(rename = "settings/context/read")]
    ContextRead { session: SessionId },
    #[serde(rename = "settings/context/set")]
    ContextSet {
        session: SessionId,
        operation: ClientOperation,
        choice: crate::context_defaults::ContextChoice,
    },
    #[serde(rename = "settings/tool-labels/set")]
    ToolLabelsSet {
        session: SessionId,
        operation: ClientOperation,
        expected: crate::ids::SettingsRevision,
        enabled: bool,
        #[serde(default)]
        save_default: bool,
    },
    #[serde(rename = "settings/set")]
    SettingsSet {
        session: SessionId,
        operation: ClientOperation,
        expected: crate::ids::SettingsRevision,
        change: crate::settings::SettingsChange,
        #[serde(default)]
        save_default: bool,
    },
    #[serde(rename = "settings/read")]
    SettingsRead {
        session: SessionId,
        expected: Option<crate::ids::SettingsRevision>,
        #[serde(default)]
        offset: u32,
        limit: Option<u16>,
    },
    #[serde(rename = "task/activity/open")]
    TaskActivityOpen {
        session: SessionId,
        task: crate::ids::TaskId,
        expected: crate::ids::TaskRevision,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        window: Option<crate::ids::TaskWindowId>,
    },
    #[serde(rename = "task/activity/read")]
    TaskActivityRead {
        session: SessionId,
        activity: crate::ids::TaskActivityId,
        since: Option<Sequence>,
    },
    #[serde(rename = "task/activity/page")]
    TaskActivityPage {
        session: SessionId,
        activity: crate::ids::TaskActivityId,
        expected: crate::ids::TaskRevision,
        after: Option<crate::ids::TaskActivityItemId>,
        limit: Option<u16>,
    },
    #[serde(rename = "task/activity/advance")]
    TaskActivityAdvance {
        session: SessionId,
        activity: crate::ids::TaskActivityId,
        older: bool,
    },
    #[serde(rename = "task/activity/close")]
    TaskActivityClose {
        session: SessionId,
        activity: Option<crate::ids::TaskActivityId>,
    },
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tool_labels: Option<bool>,
        #[serde(
            default,
            skip_serializing_if = "crate::startup::NativeLaunchOptions::is_empty"
        )]
        launch: crate::startup::NativeLaunchOptions,
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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<Box<crate::ralph::RalphOptions>>,
    },
    #[serde(rename = "session/toggleRalph")]
    ToggleRalph {
        session: SessionId,
        operation: ClientOperation,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        options: Option<Box<crate::ralph::RalphOptions>>,
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
