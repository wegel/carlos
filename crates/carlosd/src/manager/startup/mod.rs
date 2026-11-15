//! Bound private picker reads and path checks on the existing lifecycle-worker allowance.

mod open;
mod saved;
mod targets;
mod workers;

use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use carlos_core::launch_cancel::LaunchCancellation;
use carlos_core::saved_sessions::SavedPage;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{ClientId, SavedChoiceId, SavedScopeId, SessionId};
use carlos_proto::session::Backend;
use carlos_proto::startup::{NativeLaunchOptions, SavedPurpose};
use carlos_proto::submissions::ClientOperation;

use super::Manager;
use crate::transport::wire::error;

pub(in crate::manager) struct Startup {
    scopes: HashMap<u64, Scope>,
    jobs: HashMap<u64, Job>,
    next: u64,
    completed: mpsc::Receiver<Completion>,
    done: mpsc::SyncSender<Completion>,
}

struct Scope {
    id: SavedScopeId,
    backend: Backend,
    cwd: String,
    purpose: SavedPurpose,
    rows: Vec<(SavedChoiceId, carlos_core::session::models::ThreadSummary)>,
    has_newer: bool,
    has_older: bool,
}

struct Job {
    peer: u64,
    id: RequestId,
    cancel: LaunchCancellation,
    retired: bool,
    kind: JobKind,
}

enum JobKind {
    Page {
        scope: SavedScopeId,
        backend: Backend,
        purpose: SavedPurpose,
    },
    Open(Box<OpenRequest>),
}

struct OpenRequest {
    operation: ClientOperation,
    client: ClientId,
    canonical: Vec<u8>,
    target: Target,
    launch: NativeLaunchOptions,
    tool_labels: Option<bool>,
}

struct Target {
    known: Option<SessionId>,
    backend: Backend,
    cwd: String,
    native: Option<String>,
    name: Option<String>,
    requested_cwd: Option<String>,
    scope: Option<SavedScopeId>,
}

struct Completion {
    token: u64,
    result: Result<Completed, String>,
}

enum Completed {
    Page { cwd: String, page: SavedPage },
    Checked { cwd: String },
}

impl Startup {
    pub(in crate::manager) fn new() -> Self {
        let (done, completed) = mpsc::sync_channel(crate::limits::OWNERS);
        Self {
            scopes: HashMap::new(),
            jobs: HashMap::new(),
            next: 0,
            completed,
            done,
        }
    }

    pub(in crate::manager) fn close(&mut self, peer: u64) {
        self.scopes.remove(&peer);
        for job in self.jobs.values_mut().filter(|job| job.peer == peer) {
            job.retired = true;
            job.cancel.cancel();
        }
    }

    pub(in crate::manager) fn stop(&self) {
        for job in self.jobs.values() {
            job.cancel.cancel();
        }
    }
}

impl Manager {
    pub(in crate::manager) fn lifecycle_workers(&self) -> usize {
        self.startup.jobs.len()
            + self
                .sessions
                .values()
                .filter(|session| session.launch.is_some())
                .count()
    }

    pub(in crate::manager) fn startup_command(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: ClientId,
    ) {
        match command {
            Command::SavedOpen { .. } | Command::SavedPage { .. } | Command::SavedClose { .. } => {
                self.saved_command(peer, id, command)
            }
            Command::Open { .. } => self.open_command(peer, id, command, client),
            _ => unreachable!("typed startup command"),
        }
    }

    fn start_startup_job(
        &mut self,
        peer: u64,
        id: RequestId,
        kind: JobKind,
        work: impl FnOnce(&LaunchCancellation, Instant) -> Result<Completed, String> + Send + 'static,
    ) {
        if self.lifecycle_workers() >= crate::limits::OWNERS
            || self.startup.jobs.values().any(|job| job.peer == peer)
        {
            self.fail(
                peer,
                id,
                error(
                    -32010,
                    "a lifecycle read is already pending or its workers are full",
                ),
            );
            return;
        }
        let Some(token) = self.startup.next.checked_add(1) else {
            self.fail(peer, id, error(-32010, "lifecycle read counter exhausted"));
            return;
        };
        self.startup.next = token;
        let cancel = LaunchCancellation::default();
        let deadline = Instant::now() + Duration::from_secs(30);
        self.startup.jobs.insert(
            token,
            Job {
                peer,
                id,
                kind,
                cancel: cancel.clone(),
                retired: false,
            },
        );
        let done = self.startup.done.clone();
        let wake = self.wake.clone();
        thread::spawn(move || {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&cancel, deadline)))
                    .unwrap_or_else(|_| {
                        cancel.cancel();
                        Err("saved-session worker panicked".into())
                    });
            let _ = done.send(Completion { token, result });
            let _ = wake.try_send(());
        });
    }

    pub(in crate::manager) fn drain_startup(&mut self) {
        while let Ok(completion) = self.startup.completed.try_recv() {
            let Some(job) = self.startup.jobs.remove(&completion.token) else {
                continue;
            };
            if job.retired || !self.peers.contains_key(&job.peer) {
                continue;
            }
            match completion.result {
                Err(reason) => self.fail(
                    job.peer,
                    job.id,
                    error(-32012, reason.chars().take(1024).collect::<String>()),
                ),
                Ok(Completed::Page { cwd, page }) => self.finish_saved(job, cwd, page),
                Ok(Completed::Checked { cwd }) => self.finish_open(job, cwd),
            }
        }
    }
}
