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
                .filter(|session| {
                    session.saved.backend == request.target.backend
                        && (request.target.backend == carlos_proto::session::Backend::Codex
                            || session.saved.cwd == cwd)
                        && session
                            .live
                            .as_ref()
                            .map(|live| live.native_thread.as_str())
                            .or(session.saved.native_thread.as_deref())
                            == request.target.native.as_deref()
                })
                .min_by_key(|session| session.live.is_none() && session.launch.is_none())
                .map(|session| session.saved.id.clone())
        });
        let session = existing.clone().unwrap_or_else(opaque_id::<SessionId>);
        let result = match self.retry.admit(
            &request.client,
            request.operation.clone(),
            Some(session.clone()),
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
        mut request: OpenRequest,
        cwd: String,
        known: bool,
        result: OperationResult,
    ) {
        let session = result
            .session
            .clone()
            .expect("open binds a logical session");
        if known && self.sessions[&session].live.is_some() {
            if self.sessions[&session].saved.backend == carlos_proto::session::Backend::Claude
                && self.sessions[&session].info.availability == Availability::Closed
            {
                self.reopen_claude_owner(peer, id, result);
            } else {
                self.attach_open(peer, id, result);
            }
            return;
        }
        if let Some(reason) = self.open_failure(&session, known) {
            self.reject_open(peer, id, result, reason);
            return;
        }
        if !known {
            self.import_open(&session, &mut request, cwd);
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
        self.reply_open_result(peer, id, result);
    }

    fn import_open(&mut self, session: &SessionId, request: &mut OpenRequest, cwd: String) {
        let mut state = Session::dormant(SavedSession {
            id: session.clone(),
            backend: request.target.backend,
            cwd,
            name: None,
            native_thread: request.target.native.take(),
            ralph_was_on: false,
            stopped: false,
        });
        state.durable = None;
        state.info.availability = Availability::Restoring;
        state.info.name = request.target.name.take();
        self.sessions.insert(session.clone(), state);
    }

    fn reply_open_result(&mut self, peer: u64, id: RequestId, result: OperationResult) {
        let retained = self
            .retry
            .read(&result.operation.client, &result.operation, Instant::now());
        self.reply(peer, id, retained.unwrap_or(result));
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

    fn reopen_claude_owner(&mut self, peer: u64, id: RequestId, result: OperationResult) {
        let session = result.session.as_ref().expect("scoped explicit open");
        let live = self.sessions[session]
            .live
            .as_ref()
            .expect("same live owner");
        let command = carlos_core::engine::EngineCommand::Reopen {
            operation: result.operation.clone(),
            generation: None,
        };
        if let Err(reason) = live.handle.request(command) {
            self.reject_open(peer, id, result, &reason.message);
        } else {
            self.reply(peer, id, result);
        }
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
