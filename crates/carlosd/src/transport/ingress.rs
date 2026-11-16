//! Keep received command order while reserving independent count and byte limits for controls.

use std::collections::VecDeque;
use std::sync::{mpsc, Arc, Mutex};

use carlos_proto::commands::{Command, Request};

pub(crate) struct Incoming {
    pub peer: u64,
    pub request: Request,
    bytes: usize,
    class: usize,
}

#[derive(Default)]
struct State {
    queue: VecDeque<Incoming>,
    used: [(usize, usize); 10],
}

#[derive(Clone)]
pub(crate) struct Ingress {
    state: Arc<Mutex<State>>,
    wake: mpsc::SyncSender<()>,
}

impl Ingress {
    pub(crate) fn new(wake: mpsc::SyncSender<()>) -> Self {
        Self {
            state: Arc::default(),
            wake,
        }
    }

    pub(crate) fn push(&self, peer: u64, request: Request, bytes: usize) -> bool {
        self.push_in(peer, request, bytes, false)
    }

    pub(crate) fn push_network(&self, peer: u64, request: Request, bytes: usize) -> bool {
        self.push_in(peer, request, bytes, true)
    }

    fn push_in(&self, peer: u64, request: Request, bytes: usize, network: bool) -> bool {
        let kind = match request.command {
            Command::DeviceAdd(_) | Command::DeviceRevoke(_) | Command::DeviceList {} => 4,
            Command::Reopen(_)
            | Command::Create { .. }
            | Command::SavedArchive { .. }
            | Command::Open { .. }
            | Command::Send { .. }
            | Command::SetRalph { .. }
            | Command::ToggleRalph { .. }
            | Command::Rename { .. }
            | Command::GoalSet { .. }
            | Command::GoalClear { .. }
            | Command::QueueChange { .. }
            | Command::SettingsSet { .. }
            | Command::ContextSet { .. }
            | Command::ToolLabelsSet { .. }
            | Command::Rewind { .. } => 0,
            Command::TaskStop { .. }
            | Command::Interrupt { .. }
            | Command::Approve { .. }
            | Command::Reconnect { .. } => 1,
            Command::Stop(_) => 3,
            _ => 2,
        };
        let limits = [
            (64, 8 * 1024 * 1024),
            (32, 2 * 1024 * 1024),
            (128, 2 * 1024 * 1024),
            (16, 64 * 1024),
            (16, 64 * 1024),
        ];
        let class = kind + usize::from(network) * 5;
        let limit = limits[kind];
        let mut state = self.state.lock().expect("incoming commands");
        let used = &mut state.used[class];
        if used.0 == limit.0 || bytes > limit.1 - used.1 {
            return false;
        }
        used.0 += 1;
        used.1 += bytes;
        state.queue.push_back(Incoming {
            peer,
            request,
            bytes,
            class,
        });
        let _ = self.wake.try_send(());
        true
    }

    pub(crate) fn pop(&self) -> Option<Incoming> {
        let mut state = self.state.lock().expect("incoming commands");
        let incoming = state.queue.pop_front()?;
        state.used[incoming.class].0 -= 1;
        state.used[incoming.class].1 -= incoming.bytes;
        Some(incoming)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.state
            .lock()
            .expect("incoming commands")
            .queue
            .is_empty()
    }
}
