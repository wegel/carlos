//! Own one session while native workers wait and clients read bounded shared facts.

mod actions;
mod automatic;
mod backend_settings;
mod context_defaults;
mod dispatch;
mod goals;
pub mod launch;
mod mailbox;
mod native;
mod native_reads;
mod ordinary;
mod owner;
mod queue_changes;
mod queue_reads;
mod ralph_toggle;
mod reports;
mod results;
mod ring;
mod settings;
mod settle;
mod spawn;
mod target;
mod task_activity;
mod task_stops;
mod task_windows;
mod timers;
mod tool_labels;
mod workers;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use carlos_proto::commands::{ApprovalAnswer, RpcError, SendPolicy};
use carlos_proto::ids::{EntryId, Sequence, TurnId};
use carlos_proto::output::{OutputChunk, OutputCursor};
use carlos_proto::session::{History, SessionInfo, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult};

use self::mailbox::Mailbox;
pub use self::ring::EngineFrame;

/// Only explicit session reads and commands enter the owner, never native method strings.
pub enum EngineCommand {
    Prompts(crate::session::prompt_reads::PromptCommand),
    ContextRead,
    ContextSet {
        operation: ClientOperation,
        choice: carlos_proto::context_defaults::ContextChoice,
    },
    ToolLabelsSet {
        operation: ClientOperation,
        expected: carlos_proto::ids::SettingsRevision,
        enabled: bool,
        save_default: bool,
    },
    TaskActivity(crate::session::task_activity::ActivityCommand),
    SettingsSet {
        operation: ClientOperation,
        expected: carlos_proto::ids::SettingsRevision,
        change: carlos_proto::settings::SettingsChange,
        save_default: bool,
    },
    TaskWindow(crate::session::task_windows::WindowCommand),
    TaskSnapshot,
    TaskOverview {
        expected: Option<carlos_proto::ids::TaskRevision>,
        after: Option<carlos_proto::ids::TaskOverviewId>,
        limit: usize,
    },
    TaskOverviewText {
        expected: carlos_proto::ids::TaskRevision,
        entry: carlos_proto::ids::TaskOverviewId,
        field: carlos_proto::task_overview::TaskOverviewField,
        offset: carlos_proto::bytes::ByteOffset,
        limit: usize,
    },
    TaskStop {
        window: Option<crate::session::task_windows::WindowContext>,
        operation: ClientOperation,
        task: carlos_proto::ids::TaskId,
        expected_run: carlos_proto::ids::TaskRun,
    },
    TaskRun {
        window: Option<crate::session::task_windows::WindowContext>,
        task: carlos_proto::ids::TaskId,
        read: Option<carlos_proto::ids::TaskReadId>,
        refresh: bool,
    },
    TaskList {
        window: Option<crate::session::task_windows::WindowContext>,
        expected: Option<carlos_proto::ids::TaskRevision>,
        after: Option<carlos_proto::ids::TaskId>,
        limit: usize,
        refresh: bool,
    },
    TaskDetail {
        window: Option<crate::session::task_windows::WindowContext>,
        expected: carlos_proto::ids::TaskRevision,
        task: carlos_proto::ids::TaskId,
    },
    TaskText {
        window: Option<crate::session::task_windows::WindowContext>,
        expected: carlos_proto::ids::TaskRevision,
        task: carlos_proto::ids::TaskId,
        field: carlos_proto::tasks::TaskField,
        offset: carlos_proto::bytes::ByteOffset,
        limit: usize,
    },
    QueueChange {
        operation: ClientOperation,
        expected: carlos_proto::ids::QueueRevision,
        change: carlos_proto::queue::QueueChange,
    },
    QueueRead {
        expected: Option<carlos_proto::ids::QueueRevision>,
        after: Option<carlos_proto::ids::QueueEntryId>,
        limit: usize,
        refresh: bool,
    },
    QueueText {
        expected: carlos_proto::ids::QueueRevision,
        entry: carlos_proto::ids::QueueEntryId,
        offset: carlos_proto::bytes::ByteOffset,
        limit: usize,
    },
    SettingsRead {
        expected: Option<carlos_proto::ids::SettingsRevision>,
        offset: u32,
        limit: usize,
    },
    GoalRead {
        refresh: bool,
    },
    GoalSet {
        operation: ClientOperation,
        expected: carlos_proto::ids::GoalRevision,
        change: carlos_proto::goals::GoalChange,
    },
    GoalClear {
        operation: ClientOperation,
        expected: carlos_proto::ids::GoalRevision,
    },
    Info,
    Snapshot {
        limit: usize,
    },
    Changes {
        since: Option<Sequence>,
    },
    History {
        before: EntryId,
        limit: usize,
    },
    Output {
        entry: EntryId,
        cursor: Option<OutputCursor>,
        limit: usize,
    },
    Source {
        entry: EntryId,
        cursor: Option<carlos_proto::source::SourceCursor>,
        limit: usize,
    },
    SourceAt {
        entry: EntryId,
        revision: carlos_proto::output::OutputRevision,
        offset: carlos_proto::bytes::ByteOffset,
        limit: usize,
    },
    Outcomes,
    Updates {
        since: Sequence,
    },
    Send {
        operation: ClientOperation,
        text: String,
        policy: SendPolicy,
    },
    Rename {
        operation: ClientOperation,
        name: String,
    },
    Rewind {
        operation: ClientOperation,
        entry: EntryId,
        text: String,
    },
    Interrupt {
        operation: ClientOperation,
        turn: TurnId,
    },
    Approve {
        operation: ClientOperation,
        card: EntryId,
        answer: ApprovalAnswer,
    },
    SetRalph {
        operation: ClientOperation,
        on: bool,
        options: Option<Box<carlos_proto::ralph::RalphOptions>>,
    },
    ToggleRalph {
        operation: ClientOperation,
        options: Option<Box<carlos_proto::ralph::RalphOptions>>,
    },
}

/// Replies carry facts or typed failures without retaining any client's editor.
pub enum EngineReply {
    PromptSelection(carlos_proto::prompts::PromptSelection),
    PromptText(carlos_proto::prompts::PromptText),
    TaskActivity(carlos_proto::task_activity::TaskActivityHeader),
    TaskActivityPage(carlos_proto::task_activity::TaskActivityPage),
    TaskActivityClosed,
    TaskWindow(carlos_proto::task_windows::TaskWindow),
    TaskWindowClosed,
    TaskSnapshot(carlos_proto::task_facts::TaskSnapshot),
    TaskOverview(carlos_proto::task_overview::TaskOverviewPage),
    TaskOverviewText(carlos_proto::task_overview::TaskOverviewText),
    TaskRun(carlos_proto::tasks::TaskRunSnapshot),
    TaskList(carlos_proto::tasks::TaskPage),
    TaskDetail(carlos_proto::tasks::TaskDetail),
    TaskText(carlos_proto::tasks::TaskText),
    Queue(carlos_proto::queue::QueuePage),
    QueueText(carlos_proto::queue::QueueText),
    ContextDefaults(carlos_proto::context_defaults::ContextDefaults),
    Settings(carlos_proto::settings::SettingsPage),
    Goal(carlos_proto::goals::GoalSnapshot),
    Info {
        info: SessionInfo,
        native_thread: String,
    },
    Snapshot(Box<Snapshot>),
    Changes(EngineChanges),
    History(History),
    Output(OutputChunk),
    Source(carlos_proto::source::SourceChunk),
    Operation(OperationResult),
    Outcomes(Vec<OperationResult>),
    Updates(Box<EngineUpdate>),
    Failure(RpcError),
}

/// Replay keeps every sequence; a missing prefix requires a complete replacement snapshot.
pub enum EngineChanges {
    Replay {
        frames: Vec<EngineFrame>,
        seq: Sequence,
    },
    Snapshot(Box<Snapshot>),
}

/// The manager collects one ordered batch without exposing private outcomes to a device socket.
pub struct EngineUpdate {
    pub info: SessionInfo,
    pub native_thread: String,
    pub changes: Result<EngineChanges, RpcError>,
    pub outcomes: Vec<OperationResult>,
}

struct Shared {
    ordinary: Mailbox,
    control: Mailbox,
    reads: Mailbox,
    wake: mpsc::SyncSender<()>,
    stop: Arc<AtomicBool>,
    changed: Arc<AtomicBool>,
    goal_interest: Arc<AtomicBool>,
    task_interest: Arc<AtomicBool>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

/// The manager retains this handle after device sockets disconnect.
#[derive(Clone)]
pub struct EngineHandle(Arc<Shared>);

impl EngineHandle {
    /// Admit a bounded request without waiting for native work or a socket write.
    pub fn request(&self, command: EngineCommand) -> Result<mpsc::Receiver<EngineReply>, RpcError> {
        let mailbox = match &command {
            EngineCommand::Send { .. }
            | EngineCommand::SetRalph { .. }
            | EngineCommand::ToggleRalph { .. }
            | EngineCommand::Rename { .. }
            | EngineCommand::Rewind { .. } => &self.0.ordinary,
            EngineCommand::GoalSet { .. } | EngineCommand::GoalClear { .. } => &self.0.ordinary,
            EngineCommand::QueueChange { .. }
            | EngineCommand::ContextSet { .. }
            | EngineCommand::SettingsSet { .. }
            | EngineCommand::ToolLabelsSet { .. } => &self.0.ordinary,
            EngineCommand::TaskStop { .. }
            | EngineCommand::Interrupt { .. }
            | EngineCommand::Approve { .. } => &self.0.control,
            _ => &self.0.reads,
        };
        let reply = mailbox.request(command)?;
        let _ = self.0.wake.try_send(());
        Ok(reply)
    }

    /// Enable the existing native goal timer while at least one connected client reads goals.
    pub fn set_goal_interest(&self, active: bool) {
        if self.0.goal_interest.swap(active, Ordering::SeqCst) != active {
            let _ = self.0.wake.try_send(());
        }
    }

    /// Keep the existing three-second native task refresh alive for connected visible list readers.
    pub fn set_task_interest(&self, active: bool) {
        if self.0.task_interest.swap(active, Ordering::SeqCst) != active {
            let _ = self.0.wake.try_send(());
        }
    }

    /// Distinguish fact/result changes from a wake caused only by a completed read.
    pub fn take_updates(&self) -> bool {
        self.0.changed.swap(false, Ordering::SeqCst)
    }

    /// Stop the session explicitly; dropping a device socket never calls this.
    pub fn shutdown(&self) {
        self.0.stop.store(true, Ordering::SeqCst);
        let _ = self.0.wake.try_send(());
    }

    /// Wait for the already requested bounded native shutdown.
    pub fn join(&self) -> thread::Result<()> {
        if let Some(thread) = self.0.thread.lock().expect("engine join").take() {
            thread.join()
        } else {
            Ok(())
        }
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.wake.try_send(());
    }
}

impl EngineCommand {
    fn bytes(&self) -> usize {
        256 + match self {
            Self::Send { text, .. } | Self::Rewind { text, .. } => text.len(),
            Self::Rename { name, .. } => name.len(),
            Self::GoalSet { change, .. } => change.objective.as_ref().map_or(0, String::len),
            Self::QueueChange { change, .. } => change.text().map_or(0, str::len),
            Self::SettingsSet { change, .. } => {
                serde_json::to_vec(change).map_or(usize::MAX - 256, |bytes| bytes.len())
            }
            Self::Approve { answer, .. } => {
                serde_json::to_vec(answer).map_or(usize::MAX - 256, |bytes| bytes.len())
            }
            _ => 0,
        }
    }
}

#[cfg(test)]
mod owner_tests;

#[cfg(test)]
mod automatic_tests;

#[cfg(test)]
mod update_tests;

#[cfg(test)]
mod quota_tests;
