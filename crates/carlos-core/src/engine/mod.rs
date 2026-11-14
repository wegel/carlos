//! Own one session while native workers wait and clients read bounded shared facts.

mod actions;
mod automatic;
mod dispatch;
pub mod launch;
mod mailbox;
mod native;
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
    Outcomes,
    Send {
        operation: ClientOperation,
        text: String,
        policy: SendPolicy,
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
    Info {
        info: SessionInfo,
        native_thread: String,
    },
    Snapshot(Box<Snapshot>),
    Changes(EngineChanges),
    History(History),
    Output(OutputChunk),
    Operation(OperationResult),
    Outcomes(Vec<OperationResult>),
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

struct Shared {
    ordinary: Mailbox,
    control: Mailbox,
    reads: Mailbox,
    wake: mpsc::SyncSender<()>,
    stop: Arc<AtomicBool>,
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
        let channels = owner::Channels {
            ordinary: ordinary_rx,
            control: control_rx,
            reads: reads_rx,
            wake: wake.clone(),
            wake_rx,
            notice,
            stop: stop.clone(),
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
                thread: Mutex::new(Some(thread)),
            })),
            notice_rx,
        )
    }

    /// Admit a bounded request without waiting for native work or a socket write.
    pub fn request(&self, command: EngineCommand) -> Result<mpsc::Receiver<EngineReply>, RpcError> {
        let mailbox = match &command {
            EngineCommand::Send { .. } | EngineCommand::SetRalph { .. } => &self.0.ordinary,
            EngineCommand::Interrupt { .. } | EngineCommand::Approve { .. } => &self.0.control,
            _ => &self.0.reads,
        };
        let reply = mailbox.request(command)?;
        let _ = self.0.wake.try_send(());
        Ok(reply)
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
            Self::Send { text, .. } => text.len(),
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
