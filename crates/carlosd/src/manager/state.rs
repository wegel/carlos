//! Retain only launch metadata, bounded socket interests and outstanding owner reads.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc;
use std::thread::JoinHandle;

use carlos_core::engine::{EngineHandle, EngineReply};
use carlos_core::launch_cancel::LaunchCancellation;
use carlos_proto::commands::RequestId;
use carlos_proto::ids::{ClientId, EntryId, Sequence, SessionId};
use carlos_proto::session::SessionInfo;
use carlos_proto::submissions::OperationResult;

use crate::registry::SavedSession;
use crate::retry::ClientScope;
use crate::transport::PeerSocket;

pub(super) struct Peer {
    pub socket: PeerSocket,
    pub client: Option<ClientId>,
    pub scope: Option<ClientScope>,
    pub replay_allowed: bool,
    pub list: bool,
    pub sessions: HashMap<SessionId, Interest>,
    pub goals: HashSet<SessionId>,
    pub tasks: HashMap<SessionId, u64>,
    pub task_windows: HashMap<SessionId, super::task_windows::Reader>,
    pub activities: HashMap<SessionId, super::task_windows::Reader>,
    pub outputs: HashMap<(SessionId, EntryId, super::body::Format), OutputWatch>,
    pub generation: u64,
}

pub(super) struct Interest {
    pub seq: Sequence,
    pub syncing: bool,
    pub generation: u64,
}

pub(super) struct OutputWatch {
    pub cursor: super::body::Cursor,
    pub generation: u64,
    pub reading: bool,
    pub dirty: bool,
    pub done: bool,
}

pub(super) struct Session {
    pub info: SessionInfo,
    pub saved: SavedSession,
    pub durable: Option<SavedSession>,
    pub saving: Option<SavedSession>,
    pub live: Option<Live>,
    pub launch: Option<LaunchCancellation>,
    pub create: Vec<OperationResult>,
    pub save_failed: bool,
    pub stopping: Option<Stopping>,
    pub last_seq: Sequence,
}

pub(super) struct Stopping {
    pub waiters: Vec<OperationResult>,
    pub reopen: Vec<OperationResult>,
    pub worker: Option<JoinHandle<super::stop_worker::Stopped>>,
    pub joined: bool,
    pub error: Option<String>,
}

pub(super) struct Live {
    pub native_thread: String,
    pub handle: EngineHandle,
    pub notice: Option<JoinHandle<()>>,
    pub updates: Option<mpsc::Receiver<EngineReply>>,
    pub seq: Sequence,
    pub dirty: bool,
}

pub(super) struct Read {
    pub peer: u64,
    pub session: SessionId,
    pub kind: ReadKind,
    pub reply: mpsc::Receiver<EngineReply>,
    pub ready: Option<EngineReply>,
    pub reserved: usize,
}

pub(super) enum ReadKind {
    TaskSubscribe {
        id: RequestId,
        generation: u64,
    },
    Rpc(RequestId),
    Subscribe {
        id: RequestId,
        generation: u64,
    },
    Snapshot {
        generation: u64,
    },
    Output {
        entry: EntryId,
        format: super::body::Format,
        generation: u64,
    },
    OutputFirst {
        id: RequestId,
        entry: EntryId,
        format: super::body::Format,
        generation: u64,
    },
}

pub(super) struct Waiting {
    pub peer: u64,
    pub id: RequestId,
    pub session: SessionId,
    pub since: Option<Sequence>,
    pub generation: u64,
}

impl Session {
    pub(super) fn observe_owner_info(&mut self, info: SessionInfo, native_thread: &str) -> bool {
        let changed = self.info != info;
        self.info = info;
        if let Some(live) = &mut self.live {
            live.native_thread = native_thread.to_owned();
        }
        changed
    }
}
