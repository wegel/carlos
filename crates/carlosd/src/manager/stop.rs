//! Release captured native owners, then acknowledge only the durable stopped description.

use carlos_proto::commands::{Command, RequestId, SessionOperation};
use carlos_proto::ids::{ClientId, SessionId};
use carlos_proto::session::{Availability, Phase};
use carlos_proto::submissions::{OperationResult, OperationState};

use super::state::{ReadKind, Stopping};
use super::stop_worker::{self, Stopped};
use super::Manager;
use crate::retry::Class;
use crate::transport::wire::error;

impl Manager {
    pub(super) fn stop_command(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: ClientId,
    ) {
        let Command::Stop(SessionOperation { session, .. }) = &command else {
            unreachable!("typed session stop");
        };
        let session = session.clone();
        let Some(result) =
            self.admit_lifecycle(peer, id.clone(), &command, &client, Class::Lifecycle)
        else {
            return;
        };
        if !self.sessions.contains_key(&session) {
            self.reject_lifecycle(peer, id, result, "Unknown session; nothing stopped");
            return;
        }
        if let Some(stop) = &mut self.sessions.get_mut(&session).expect("session").stopping {
            if stop.waiters.len() >= 64 {
                self.reject_lifecycle(peer, id, result, "Too many pending stops for this session");
                return;
            }
            stop.waiters.push(result.clone());
        } else {
            self.begin_stop(&session, vec![result.clone()], Vec::new());
        }
        self.reply(peer, id, result);
    }

    pub(super) fn begin_stop(
        &mut self,
        id: &SessionId,
        waiters: Vec<OperationResult>,
        reopen: Vec<OperationResult>,
    ) {
        self.cancel_owner_reads(id);
        let state = self.sessions.get_mut(id).expect("known session");
        if let Some(cancel) = &state.launch {
            cancel.cancel();
        }
        state.saved.stopped = true;
        state.saved.ralph_was_on = false;
        let worker = state
            .live
            .as_mut()
            .map(|live| stop_worker::start(live, self.wake.clone()));
        let joined = worker.is_none() && state.launch.is_none();
        state.stopping = Some(Stopping {
            waiters,
            reopen,
            worker,
            joined,
            error: None,
        });
        if joined {
            self.owner_stopped(
                id,
                Stopped {
                    updates: Vec::new(),
                    error: None,
                },
            );
        }
    }

    pub(super) fn drain_stops(&mut self) {
        let ready: Vec<_> = self
            .sessions
            .iter()
            .filter(|(_, state)| {
                state
                    .stopping
                    .as_ref()
                    .and_then(|stop| stop.worker.as_ref())
                    .is_some_and(std::thread::JoinHandle::is_finished)
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in ready {
            let worker = self
                .sessions
                .get_mut(&id)
                .expect("session")
                .stopping
                .as_mut()
                .expect("stopping")
                .worker
                .take()
                .expect("worker");
            let stopped = worker.join().unwrap_or_else(|_| Stopped {
                updates: Vec::new(),
                error: Some("Session stop worker panicked".into()),
            });
            self.owner_stopped(&id, stopped);
        }
    }

    pub(super) fn owner_stopped(&mut self, id: &SessionId, stopped: Stopped) {
        for update in stopped.updates {
            let state = self.sessions.get_mut(id).expect("session");
            if !update.native_thread.is_empty() {
                state.saved.native_thread = Some(update.native_thread.clone());
            }
            self.apply_update(id, update);
        }
        let state = self.sessions.get_mut(id).expect("session");
        state.live = None;
        state.saved.stopped = true;
        state.saved.ralph_was_on = false;
        state.info.availability = Availability::Closed;
        state.info.phase = Phase::Idle;
        state.info.current_turn = None;
        state.info.ralph = false;
        state.info.turn_started_at = None;
        let stop = state.stopping.as_mut().expect("stopping owner");
        stop.joined = true;
        stop.error = stopped.error;
        let waiters = stop
            .waiters
            .iter()
            .chain(&stop.reopen)
            .map(|result| result.operation.clone())
            .collect::<Vec<_>>();
        for result in self.retry.stopped(id, &waiters) {
            self.result(result);
        }
        self.sessions.get_mut(id).expect("session").create.clear();
        self.list_changed(id);
    }

    pub(super) fn finish_stops(&mut self) {
        let ready: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(id, state)| {
                let stop = state.stopping.as_ref()?;
                (stop.joined
                    && (state.save_failed
                        || self.store.failed
                        || state.durable.as_ref() == Some(&state.saved)))
                .then(|| id.clone())
            })
            .collect();
        for id in ready {
            let state = self.sessions.get_mut(&id).expect("session");
            let stop = state.stopping.take().expect("stop");
            let reason = if state.save_failed || self.store.failed {
                Some("Session owner stopped, but its catalogue save was not confirmed".to_owned())
            } else {
                stop.error
            };
            for mut result in stop.waiters {
                result.state = match &reason {
                    Some(message) => OperationState::Uncertain {
                        message: message.clone(),
                        recovery: None,
                    },
                    None => OperationState::Accepted {
                        entry: None,
                        notice: Some("Session stopped; saved conversation kept".into()),
                    },
                };
                self.result(result);
            }
            for result in stop.reopen {
                if let Some(reason) = &reason {
                    self.fail_reopen(result, reason);
                } else {
                    self.begin_reopen(result);
                }
            }
        }
    }

    fn cancel_owner_reads(&mut self, id: &SessionId) {
        let mut keep = Vec::new();
        for read in std::mem::take(&mut self.reads) {
            if &read.session != id {
                keep.push(read);
                continue;
            }
            self.read_bytes -= read.reserved;
            let request = match read.kind {
                ReadKind::Rpc(id)
                | ReadKind::TaskSubscribe { id, .. }
                | ReadKind::Subscribe { id, .. }
                | ReadKind::OutputFirst { id, .. } => Some(id),
                _ => None,
            };
            if let Some(request) = request {
                self.fail(
                    read.peer,
                    request,
                    error(-32012, "Session is stopping; this read was cancelled"),
                );
            }
        }
        self.reads = keep;
        for waiting in std::mem::take(&mut self.waiting) {
            if &waiting.session == id {
                self.fail(
                    waiting.peer,
                    waiting.id,
                    error(
                        -32012,
                        "Session is stopping; subscribe after explicit reopen",
                    ),
                );
            } else {
                self.waiting.push(waiting);
            }
        }
        for peer in self.peers.values_mut() {
            if let Some(interest) = peer.sessions.get_mut(id) {
                interest.syncing = false;
            }
            peer.outputs.retain(|(session, _, _), _| session != id);
            peer.task_windows.remove(id);
            peer.activities.remove(id);
        }
    }
}
