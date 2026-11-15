//! Admit typed commands and fan out owner facts without waiting for native, disk or socket I/O.

mod body;
mod commands;
mod goals;
mod lifecycle;
mod mutation_work;
mod output;
mod owner_launch;
mod prompt_reads;
mod read_commands;
mod reads;
mod reopen;
mod restore_admission;
mod shutdown;
mod startup;
mod state;
mod stop;
mod stop_worker;
mod store;
mod task_activity;
mod task_windows;
mod tasks;
mod updates;

use std::collections::HashMap;
use std::fs::File;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use carlos_core::engine::launch::{LaunchError, Launched};
use carlos_proto::commands::{RequestId, RpcError};
use carlos_proto::events::Notification;
use carlos_proto::ids::{DaemonId, SessionId};
use carlos_proto::submissions::OperationResult;
use serde::Serialize;

use crate::registry::RegistryError;
use crate::retry::RetryStore;
use crate::transport::ingress::Ingress;
use crate::transport::outbox::{Frame, Kind};
use crate::transport::{wire, PeerSocket};

use self::state::{Peer, Read, Session, Waiting};
use self::store::Store;

pub(crate) struct Manager {
    peers: HashMap<u64, Peer>,
    sessions: HashMap<SessionId, Session>,
    startup: startup::Startup,
    reads: Vec<Read>,
    read_bytes: usize,
    waiting: Vec<Waiting>,
    retry: RetryStore,
    store: Store,
    ingress: Ingress,
    wake: mpsc::SyncSender<()>,
    wake_rx: mpsc::Receiver<()>,
    launches: mpsc::Receiver<(SessionId, Result<Launched, LaunchError>)>,
    launch_tx: mpsc::SyncSender<(SessionId, Result<Launched, LaunchError>)>,
    codex_socket: Option<PathBuf>,
    next_peer: u64,
}

impl Manager {
    pub(crate) fn new(
        state: PathBuf,
        codex_socket: Option<PathBuf>,
        state_guard: File,
    ) -> Result<Self, RegistryError> {
        let (wake, wake_rx) = mpsc::sync_channel(1);
        let (launch_tx, launches) = mpsc::sync_channel(16);
        let (store, saved) =
            Store::start(state.join("sessions.sqlite3"), wake.clone(), state_guard)?;
        let sessions = saved
            .into_iter()
            .map(|saved| (saved.id.clone(), Session::dormant(saved)))
            .collect();
        Ok(Self {
            peers: HashMap::new(),
            sessions,
            startup: startup::Startup::new(),
            reads: Vec::new(),
            read_bytes: 0,
            waiting: Vec::new(),
            retry: RetryStore::new(
                uuid::Uuid::new_v4()
                    .simple()
                    .to_string()
                    .parse::<DaemonId>()
                    .expect("UUID"),
            ),
            store,
            ingress: Ingress::new(wake.clone()),
            wake,
            wake_rx,
            launches,
            launch_tx,
            codex_socket,
            next_peer: 0,
        })
    }

    pub(crate) fn wake(&self) -> mpsc::SyncSender<()> {
        self.wake.clone()
    }

    pub(crate) fn run(&mut self, accepted: mpsc::Receiver<UnixStream>, stop: Arc<AtomicBool>) {
        while !stop.load(Ordering::SeqCst) {
            while let Ok(stream) = accepted.try_recv() {
                self.accept(stream);
            }
            self.remove_closed();
            for _ in 0..224 {
                let Some(incoming) = self.ingress.pop() else {
                    break;
                };
                self.command(incoming.peer, incoming.request);
            }
            self.drain_launches();
            self.drain_startup();
            self.refresh_goal_interests();
            self.refresh_task_interests();
            self.drain_updates();
            self.drain_reads();
            self.drain_stops();
            self.drain_saves();
            self.finish_stops();
            self.schedule_updates();
            self.schedule_catchups();
            self.schedule_outputs();
            self.schedule_saves();
            let _ = self.wake_rx.recv_timeout(Duration::from_millis(250));
        }
        self.shutdown();
    }

    fn accept(&mut self, stream: UnixStream) {
        if self.peers.len() == crate::limits::PEERS {
            return;
        }
        let Some(id) = self.next_peer.checked_add(1) else {
            return;
        };
        self.next_peer = id;
        if let Ok(socket) = PeerSocket::start(id, stream, self.ingress.clone(), self.wake.clone()) {
            self.peers.insert(
                id,
                Peer {
                    socket,
                    client: None,
                    replay_allowed: false,
                    list: false,
                    sessions: HashMap::new(),
                    goals: Default::default(),
                    tasks: Default::default(),
                    task_windows: Default::default(),
                    activities: Default::default(),
                    outputs: HashMap::new(),
                    generation: 0,
                },
            );
        }
    }

    fn remove_closed(&mut self) {
        let closed: Vec<_> = self
            .peers
            .iter()
            .filter(|(_, peer)| peer.socket.closed.load(Ordering::SeqCst))
            .map(|(id, _)| *id)
            .collect();
        for id in closed {
            self.startup.close(id);
            if let Some(client) = self.peers.remove(&id).and_then(|peer| peer.client.clone()) {
                self.retry.disconnect(&client, Instant::now());
            }
            self.reads.retain(|read| {
                if read.peer != id {
                    return true;
                }
                self.read_bytes -= read.reserved;
                false
            });
            self.waiting.retain(|read| read.peer != id);
        }
    }

    fn reply(&self, peer: u64, id: RequestId, result: impl Serialize) {
        self.required(
            peer,
            wire::success(id.clone(), result).or_else(|_| {
                wire::encode(&wire::failure(
                    Some(id),
                    -32011,
                    "reply exceeds the eight MiB frame budget",
                ))
            }),
        );
    }

    fn fail(&self, peer: u64, id: RequestId, error: RpcError) {
        self.required(
            peer,
            wire::encode(&wire::failure(Some(id), error.code, &error.message)),
        );
    }

    fn required(&self, peer: u64, bytes: io::Result<Arc<[u8]>>) {
        if let Some(peer) = self.peers.get(&peer) {
            if !bytes.is_ok_and(|bytes| {
                peer.socket.outbox.push(Frame {
                    bytes,
                    kind: Kind::Reply,
                })
            }) {
                peer.socket.close();
            }
        }
    }

    fn result(&mut self, result: OperationResult) {
        match self.retry.complete(result) {
            Ok(Some(result)) => {
                let client = result.operation.client.clone();
                let bytes = wire::notification(Notification::Operation(result));
                for (id, peer) in &self.peers {
                    if peer.client.as_ref() == Some(&client) {
                        self.required(
                            *id,
                            bytes
                                .as_ref()
                                .map(Arc::clone)
                                .map_err(|_| io::Error::other("result encoding failed")),
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(error) => {
                // A retained result must never silently disappear behind an internal budget error.
                eprintln!("carlosd: retained result failed: {}", error.message);
                for peer in self.peers.values() {
                    peer.socket.close();
                }
            }
        }
    }
}
