//! Launch once per logical session and acknowledge creation only after saving its native address.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use carlos_core::engine::launch::{
    launch, LaunchError, LaunchOptions, LaunchRequest, RestoreSession,
};
use carlos_core::launch_cancel::LaunchCancellation;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::events::{Notification, SessionChange};
use carlos_proto::ids::{ClientId, Sequence, SessionId};
use carlos_proto::session::Availability;
use carlos_proto::submissions::{OperationResult, OperationState};

use super::state::{Live, Session};
use super::Manager;
use crate::registry::SavedSession;
use crate::retry::{Admission, Class};
use crate::transport::wire;

impl Manager {
    pub(super) fn create(&mut self, peer: u64, id: RequestId, command: Command, client: ClientId) {
        let canonical = serde_json::to_vec(&command).expect("typed command");
        let Command::Create {
            operation,
            backend,
            cwd,
            name,
            tool_labels,
            launch,
        } = command
        else {
            unreachable!()
        };
        let session = new_session_id();
        let result = match self.retry.admit(
            &client,
            operation,
            Some(session.clone()),
            canonical,
            Class::Ordinary,
            Instant::now(),
        ) {
            Ok(Admission::Existing(result)) => {
                self.reply(peer, id, result);
                return;
            }
            Ok(Admission::New(result)) => result,
            Err(reason) => {
                self.fail(peer, id, reason);
                return;
            }
        };
        if let Some(message) = self.create_failure(&cwd, &name) {
            let result = OperationResult {
                state: OperationState::Rejected {
                    message: message.into(),
                    recovery: None,
                },
                ..result
            };
            self.result(result.clone());
            self.reply(peer, id, result);
            return;
        }
        let saved = SavedSession {
            id: session.clone(),
            backend,
            cwd,
            name,
            native_thread: None,
            ralph_was_on: false,
        };
        self.launch_created(saved, result.clone(), tool_labels, launch);
        self.reply(peer, id, result);
    }

    fn launch_created(
        &mut self,
        saved: SavedSession,
        result: OperationResult,
        tool_labels: Option<bool>,
        launch: carlos_proto::startup::NativeLaunchOptions,
    ) {
        let session = saved.id.clone();
        let mut state = Session::dormant(saved);
        state.durable = None;
        state.info.availability = Availability::Starting;
        state.create.push(result.clone());
        self.sessions.insert(session.clone(), state);
        self.start_launch(&session, false, tool_labels, launch);
        self.list_changed(&session);
    }

    fn create_failure(&self, cwd: &str, name: &Option<String>) -> Option<&'static str> {
        if self.store.failed {
            return Some("catalogue worker is unavailable; nothing launched");
        }
        if self.sessions.len() == crate::limits::SESSIONS
            || self.owner_count() == crate::limits::OWNERS
            || self.lifecycle_workers() >= crate::limits::OWNERS
        {
            Some("session capacity is full; nothing launched")
        } else if !PathBuf::from(cwd).is_absolute()
            || cwd.len() > 16384
            || name.as_ref().is_some_and(|name| name.len() > 4096)
        {
            Some("invalid working directory or oversized name; nothing launched")
        } else {
            None
        }
    }

    pub(super) fn start_launch(
        &mut self,
        id: &SessionId,
        restore: bool,
        tool_labels: Option<bool>,
        native: carlos_proto::startup::NativeLaunchOptions,
    ) {
        if restore && self.reject_restore_conflict(id) {
            return;
        }
        let state = self.sessions.get_mut(id).expect("known session");
        if state.live.is_some() || state.launch.is_some() {
            return;
        }
        let cancel = LaunchCancellation::default();
        let saved = &state.saved;
        let request = LaunchRequest {
            session: id.clone(),
            backend: saved.backend,
            cwd: PathBuf::from(&saved.cwd),
            name: saved.name.clone(),
            options: LaunchOptions {
                codex_socket: self.codex_socket.clone(),
                tool_labels,
                native,
            },
            restore: restore.then(|| RestoreSession {
                native_thread: saved.native_thread.clone().expect("validated restore"),
                ralph_was_on: saved.ralph_was_on,
            }),
        };
        state.launch = Some(cancel.clone());
        let done = self.launch_tx.clone();
        let wake = self.wake.clone();
        let id = id.clone();
        thread::spawn(move || {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| launch(request, &cancel)))
                    .unwrap_or_else(|_| {
                        cancel.cancel();
                        Err(LaunchError::Native("native launch worker panicked".into()))
                    });
            let _ = done.send((id, result));
            let _ = wake.try_send(());
        });
    }

    pub(super) fn owner_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|session| session.live.is_some() || session.launch.is_some())
            .count()
    }

    pub(super) fn drain_launches(&mut self) {
        while let Ok((id, result)) = self.launches.try_recv() {
            let Some(session) = self.sessions.get_mut(&id) else {
                continue;
            };
            session.launch = None;
            match result {
                Ok(mut launched) => {
                    session.info = launched.info();
                    let native_thread = launched.native_thread().to_owned();
                    let (handle, notices) = launched.spawn();
                    let wake = self.wake.clone();
                    let notice = thread::spawn(move || {
                        while notices.recv().is_ok() {
                            let _ = wake.try_send(());
                        }
                    });
                    session.live = Some(Live {
                        native_thread,
                        handle,
                        notice,
                        updates: None,
                        seq: Sequence::default(),
                        dirty: true,
                    });
                }
                Err(reason) => {
                    session.info.availability = Availability::Unavailable;
                    for mut result in std::mem::take(&mut session.create) {
                        result.state = match &reason {
                            LaunchError::Invalid(message) => OperationState::Rejected {
                                message: message.clone(),
                                recovery: None,
                            },
                            reason => OperationState::Uncertain {
                                message: reason.to_string(),
                                recovery: None,
                            },
                        };
                        self.result(result);
                    }
                }
            }
            self.list_changed(&id);
            self.resume_subscriptions(&id);
        }
    }

    pub(super) fn schedule_saves(&mut self) {
        for session in self.sessions.values_mut() {
            if session.saving.is_none()
                && !session.save_failed
                && (session.durable.is_some()
                    || session.saved.native_thread.is_none()
                    || session.info.availability == Availability::Ready)
                && session.durable.as_ref() != Some(&session.saved)
                && self.store.save(session.saved.clone())
            {
                session.saving = Some(session.saved.clone());
            }
        }
    }

    pub(super) fn drain_saves(&mut self) {
        loop {
            let saved = match self.store.take() {
                Ok(Some(saved)) => saved,
                Ok(None) => break,
                Err(()) => {
                    self.catalogue_failed();
                    break;
                }
            };
            let id = saved.description.id.clone();
            let Some(session) = self.sessions.get_mut(&id) else {
                continue;
            };
            session.saving = None;
            match saved.result {
                Ok(()) => session.durable = Some(saved.description),
                Err(reason) => {
                    session.save_failed = true;
                    for mut result in std::mem::take(&mut session.create) {
                        result.state = OperationState::Uncertain {
                            message: format!(
                                "backend may be open but catalogue save failed: {reason}"
                            ),
                            recovery: None,
                        };
                        self.result(result);
                    }
                    eprintln!("carlosd: cannot save session {id}: {reason}");
                }
            }
            self.finish_create(&id);
        }
    }

    fn catalogue_failed(&mut self) {
        eprintln!("carlosd: catalogue worker stopped; no further create will launch a backend");
        let mut results = Vec::new();
        for session in self.sessions.values_mut() {
            session.saving = None;
            session.save_failed = true;
            for mut result in std::mem::take(&mut session.create) {
                result.state = OperationState::Uncertain {
                    message: "backend may be open but catalogue worker stopped".into(),
                    recovery: None,
                };
                results.push(result);
            }
        }
        for result in results {
            self.result(result);
        }
    }

    pub(super) fn list_changed(&self, id: &SessionId) {
        let info = self.sessions[id].info.clone();
        let bytes = wire::notification(Notification::SessionsChanged(SessionChange::Updated {
            info,
        }));
        for (id, peer) in &self.peers {
            if peer.list {
                self.required(
                    *id,
                    bytes
                        .as_ref()
                        .map(std::sync::Arc::clone)
                        .map_err(|_| std::io::Error::other("session list encoding failed")),
                );
            }
        }
    }

    pub(super) fn shutdown(&mut self) {
        self.startup.stop();
        for session in self.sessions.values() {
            if let Some(cancel) = &session.launch {
                cancel.cancel();
            }
            if let Some(live) = &session.live {
                live.handle.shutdown();
            }
        }
        for peer in self.peers.values() {
            peer.socket.close();
        }
        self.peers.clear();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.drain_saves();
            self.schedule_saves();
            if self
                .sessions
                .values()
                .all(|session| session.saving.is_none())
                || Instant::now() >= deadline
            {
                break;
            }
            let _ = self.wake_rx.recv_timeout(Duration::from_millis(25));
        }
        for session in self.sessions.values_mut() {
            if let Some(live) = session.live.take() {
                let _ = live.handle.join();
                let _ = live.notice.join();
            }
        }
    }
}

impl Session {
    pub(super) fn dormant(saved: SavedSession) -> Self {
        Self {
            info: saved.dormant_info(),
            durable: Some(saved.clone()),
            saved,
            saving: None,
            live: None,
            launch: None,
            create: Vec::new(),
            save_failed: false,
        }
    }
}

fn new_session_id() -> SessionId {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .parse()
        .expect("UUID")
}
