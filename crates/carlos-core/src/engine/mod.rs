//! Own one session while native workers wait and clients read bounded shared facts.

mod actions;
mod automatic;
mod dispatch;
mod goals;
pub mod launch;
mod mailbox;
mod native;
mod ordinary;
mod owner;
mod reports;
mod results;
mod ring;
mod settle;
mod target;
mod timers;
mod workers;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;

use carlos_proto::commands::{ApprovalAnswer, RpcError, SendPolicy};
use carlos_proto::ids::{EntryId, Sequence, TurnId};
use carlos_proto::output::{OutputChunk, OutputCursor};
use carlos_proto::session::{History, SessionInfo, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult};

use crate::backend::{BackendClient, EventReceiver};
use crate::session::state::SessionState;

use self::mailbox::Mailbox;
pub use self::ring::EngineFrame;

/// Only explicit session reads and commands enter the owner, never native method strings.
pub enum EngineCommand {
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
    },
}

/// Replies carry facts or typed failures without retaining any client's editor.
pub enum EngineReply {
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
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

/// The manager retains this handle after device sockets disconnect.
#[derive(Clone)]
pub struct EngineHandle(Arc<Shared>);

impl EngineHandle {
    /// Transfer a backend, its event consumer and the session to one owner thread.
    pub fn spawn(
        session: SessionState,
        client: Box<dyn BackendClient>,
        events: EventReceiver,
    ) -> (Self, mpsc::Receiver<()>) {
        let (ordinary, ordinary_rx) = Mailbox::new(64, 8 * 1024 * 1024);
        let (control, control_rx) = Mailbox::new(16, 2 * 1024 * 1024);
        let (reads, reads_rx) = Mailbox::new(64, 1024 * 1024);
        let (wake, wake_rx) = mpsc::sync_channel(1);
        let (notice, notice_rx) = mpsc::sync_channel(1);
        let stop = Arc::new(AtomicBool::new(false));
        let changed = Arc::new(AtomicBool::new(true));
        let channels = owner::Channels {
            ordinary: ordinary_rx,
            control: control_rx,
            reads: reads_rx,
            wake: wake.clone(),
            wake_rx,
            notice,
            stop: stop.clone(),
            changed: changed.clone(),
        };
        let owner = owner::Owner::new(session, client, events, channels);
        let thread = thread::spawn(move || owner.run());
        (
            Self(Arc::new(Shared {
                ordinary,
                control,
                reads,
                wake,
                stop,
                changed,
                thread: Mutex::new(Some(thread)),
            })),
            notice_rx,
        )
    }

    /// Admit a bounded request without waiting for native work or a socket write.
    pub fn request(&self, command: EngineCommand) -> Result<mpsc::Receiver<EngineReply>, RpcError> {
        let mailbox = match &command {
            EngineCommand::Send { .. }
            | EngineCommand::SetRalph { .. }
            | EngineCommand::Rename { .. }
            | EngineCommand::Rewind { .. } => &self.0.ordinary,
            EngineCommand::GoalSet { .. } | EngineCommand::GoalClear { .. } => &self.0.ordinary,
            EngineCommand::Interrupt { .. } | EngineCommand::Approve { .. } => &self.0.control,
            _ => &self.0.reads,
        };
        let reply = mailbox.request(command)?;
        let _ = self.0.wake.try_send(());
        Ok(reply)
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
