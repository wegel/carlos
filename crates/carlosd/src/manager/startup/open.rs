//! Resolve explicit imports before admission so concurrent clients share one logical/native owner.

use std::time::Instant;

use carlos_proto::commands::{Command, RequestId};
use carlos_proto::ids::{ClientId, SessionId};
use carlos_proto::session::Availability;
use carlos_proto::startup::NativeLaunchOptions;
use carlos_proto::submissions::{OperationResult, OperationState};

use super::saved::opaque_id;
use super::{workers, Completed, Job, JobKind, OpenRequest};
use crate::manager::state::Session;
use crate::manager::Manager;
use crate::registry::SavedSession;
use crate::retry::{Admission, Class};

impl Manager {
    pub(super) fn open_command(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: ClientId,
    ) {
        let canonical = serde_json::to_vec(&command).expect("typed open");
        let Command::Open {
            operation,
            backend,
            cwd,
            target,
            launch,
            tool_labels,
        } = command
        else {
            unreachable!("typed open")
        };
        match self
            .retry
            .existing(&client, &operation, &canonical, Instant::now())
        {
            Ok(Some(result)) => {
                self.reply(peer, id, result);
                return;
            }
            Ok(None) => {}
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        }
        let target = match self.resolve_target(peer, backend, cwd, target) {
            Ok(target) => target,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        let request = OpenRequest {
            operation,
            client,
            canonical,
            target,
            launch,
            tool_labels,
        };
        self.queue_open(peer, id, request);
    }

    fn queue_open(&mut self, peer: u64, id: RequestId, request: OpenRequest) {
        let requested = request.launch.codex_socket.clone();
        let configured = self.codex_socket.clone();
        let cwd = request.target.cwd.clone();
        let backend = request.target.backend;
        let requested_cwd = request.target.requested_cwd.clone();
        self.start_startup_job(
            peer,
            id,
            JobKind::Open(Box::new(request)),
            move |cancel, deadline| {
                let cwd = workers::check(
                    backend,
                    &cwd,
                    requested.as_deref(),
                    configured.as_deref(),
                    cancel,
                    deadline,
                )?;
                if let Some(requested_cwd) = requested_cwd {
                    let checked =
                        workers::check(backend, &requested_cwd, None, None, cancel, deadline)?;
                    if checked != cwd {
                        return Err(
                            "saved-session choice belongs to another working directory".into()
                        );
                    }
                }
                Ok(Completed::Checked { cwd })
            },
        );
    }

    pub(super) fn finish_open(&mut self, job: Job, cwd: String) {
        let JobKind::Open(request) = job.kind else {
            unreachable!("checked open")
        };
        let mut request = *request;
        let existing = request.target.known.clone().or_else(|| {
            self.sessions
                .values()
                .find(|session| {
                    session.saved.backend == request.target.backend
                        && session.saved.cwd == cwd
                        && session.saved.native_thread == request.target.native
                })
                .map(|session| session.saved.id.clone())
        });
        let session = existing.clone().unwrap_or_else(opaque_id::<SessionId>);
        let result = match self.retry.admit(
            &request.client,
            request.operation.clone(),
            session.clone(),
            std::mem::take(&mut request.canonical),
            Class::Ordinary,
            Instant::now(),
        ) {
            Ok(Admission::Existing(result)) => {
                self.reply(job.peer, job.id, result);
                return;
            }
            Ok(Admission::New(result)) => result,
            Err(reason) => {
                self.fail(job.peer, job.id, reason);
                return;
            }
        };
        self.install_open(job.peer, job.id, request, cwd, existing.is_some(), result);
    }

    fn install_open(
        &mut self,
        peer: u64,
        id: RequestId,
        request: OpenRequest,
        cwd: String,
        known: bool,
        result: OperationResult,
    ) {
        let session = result.session.clone();
        if known && self.sessions[&session].live.is_some() {
            self.attach_open(peer, id, result);
            return;
        }
        if let Some(reason) = self.open_failure(&session, known) {
            self.reject_open(peer, id, result, reason);
            return;
        }
        if !known {
            let mut state = Session::dormant(SavedSession {
                id: session.clone(),
                backend: request.target.backend,
                cwd,
                name: None,
                native_thread: request.target.native,
                ralph_was_on: false,
            });
            state.durable = None;
            state.info.availability = Availability::Restoring;
            state.info.name = request.target.name;
            self.sessions.insert(session.clone(), state);
        }
        let state = self
            .sessions
            .get_mut(&session)
            .expect("selected logical session");
        state.create.push(result.clone());
        if state.live.is_none() && state.launch.is_none() {
            state.info.availability = Availability::Restoring;
            let launch = if !known {
                request.launch
            } else {
                NativeLaunchOptions::default()
            };
            let labels = if !known { request.tool_labels } else { None };
            self.start_launch(&session, true, labels, launch);
        }
        self.finish_create(&session);
        self.list_changed(&session);
        self.reply(peer, id, result);
    }

    fn open_failure(&self, id: &SessionId, known: bool) -> Option<&'static str> {
        if self.store.failed {
            return Some("catalogue worker is unavailable; nothing opened");
        }
        if !known && self.sessions.len() >= crate::limits::SESSIONS {
            return Some("saved-session capacity is full; nothing opened");
        }
        if let Some(state) = self.sessions.get(id) {
            if state.save_failed {
                return Some("catalogue save failed; nothing reopened");
            }
            if state.create.len() >= 64 {
                return Some("too many clients are waiting for this native open");
            }
            if state.saved.native_thread.is_none() && state.live.is_none() && state.launch.is_none()
            {
                return Some(
                    "this logical session has no confirmed native address; nothing opened",
                );
            }
            if state.live.is_some() || state.launch.is_some() {
                return None;
            }
        }
        if self.owner_count() >= crate::limits::OWNERS
            || self.lifecycle_workers() >= crate::limits::OWNERS
        {
            return Some("native owner or lifecycle capacity is full; nothing opened");
        }
        None
    }

    fn reject_open(&mut self, peer: u64, id: RequestId, mut result: OperationResult, reason: &str) {
        result.state = OperationState::Rejected {
            message: reason.into(),
            recovery: None,
        };
        self.result(result.clone());
        self.reply(peer, id, result);
    }

    fn attach_open(&mut self, peer: u64, id: RequestId, mut result: OperationResult) {
        result.state = OperationState::Accepted {
            entry: None,
            notice: Some("Attached existing owner; first-launch choices remain unchanged".into()),
        };
        self.result(result.clone());
        self.reply(peer, id, result);
    }
}
