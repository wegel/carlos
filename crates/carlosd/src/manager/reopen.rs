//! Retain exact lifecycle retries before inspecting or replacing a native owner.

use std::time::Instant;

use carlos_proto::commands::{Command, RequestId, SessionOperation};
use carlos_proto::ids::ClientId;
use carlos_proto::session::Availability;
use carlos_proto::submissions::{OperationResult, OperationState};

use super::Manager;
use crate::command_info::describe;
use crate::retry::{Admission, Class};

impl Manager {
    pub(super) fn admit_lifecycle(
        &mut self,
        peer: u64,
        id: RequestId,
        command: &Command,
        client: &ClientId,
        class: Class,
    ) -> Option<OperationResult> {
        let (Command::Stop(request) | Command::Reopen(request)) = command else {
            unreachable!("lifecycle request");
        };
        let admitted = self.retry.admit(
            client,
            request.operation.clone(),
            Some(request.session.clone()),
            serde_json::to_vec(command).expect("typed lifecycle"),
            class,
            Instant::now(),
        );
        match admitted {
            Ok(Admission::New(result)) => Some(result),
            Ok(Admission::Existing(result)) => {
                self.reply(peer, id, result);
                None
            }
            Err(reason) => {
                self.fail(peer, id, reason);
                None
            }
        }
    }

    pub(super) fn reopen_command(
        &mut self,
        peer: u64,
        id: RequestId,
        command: Command,
        client: ClientId,
    ) {
        let Command::Reopen(SessionOperation { session, .. }) = &command else {
            unreachable!("reopen");
        };
        let session = session.clone();
        let Some(mut result) = self.admit_lifecycle(
            peer,
            id.clone(),
            &command,
            &client,
            describe(&command).retry.expect("reopen retry class"),
        ) else {
            return;
        };
        let reason = self.reopen_failure(&session);
        if let Some(reason) = reason {
            self.reject_lifecycle(peer, id, result, reason);
            return;
        }
        let state = &self.sessions[&session];
        if state.live.is_some() && state.info.availability == Availability::Ready {
            result.state = OperationState::Accepted {
                entry: None,
                notice: Some("Session already open".into()),
            };
            self.result(result.clone());
        } else if state.live.is_some()
            && matches!(
                state.info.availability,
                Availability::Closed | Availability::Unavailable
            )
        {
            self.begin_stop(&session, Vec::new(), vec![result.clone()]);
        } else {
            self.begin_reopen(result.clone());
        }
        self.reply(peer, id, result);
    }

    fn reopen_failure(&self, id: &carlos_proto::ids::SessionId) -> Option<&'static str> {
        let Some(state) = self.sessions.get(id) else {
            return Some("Unknown session; nothing reopened");
        };
        if state.stopping.is_some() {
            return Some("Session is still stopping; retry reopen after stop finishes");
        }
        if self.store.failed || state.save_failed {
            return Some("Catalogue unavailable; nothing reopened");
        }
        if state.create.len() >= 64 {
            return Some("Too many pending opens for this session");
        }
        if state.live.is_some() || state.launch.is_some() {
            return None;
        }
        if state.last_seq.checked_next().is_none() {
            return Some("Session event counter exhausted; restart the daemon before reopening");
        }
        if state.saved.native_thread.is_none() {
            return Some("No confirmed native address; no replacement was created");
        }
        if self.owner_count() >= crate::limits::OWNERS
            || self.lifecycle_workers() >= crate::limits::OWNERS
        {
            return Some("Native owner capacity is full; nothing reopened");
        }
        None
    }

    pub(super) fn begin_reopen(&mut self, result: OperationResult) {
        let id = result.session.clone().expect("scoped reopen");
        if let Some(reason) = self.reopen_failure(&id) {
            self.fail_reopen(result, reason);
            return;
        }
        let state = self.sessions.get_mut(&id).expect("session");
        state.create.push(result);
        if state.live.is_none() && state.launch.is_none() {
            state.info.availability = Availability::Restoring;
            self.start_launch(&id, true, None, Default::default());
            self.list_changed(&id);
        }
    }

    pub(super) fn fail_reopen(&mut self, mut result: OperationResult, reason: &str) {
        result.state = OperationState::Uncertain {
            message: reason.into(),
            recovery: None,
        };
        self.result(result);
    }

    pub(super) fn reject_lifecycle(
        &mut self,
        peer: u64,
        id: RequestId,
        mut result: OperationResult,
        reason: &str,
    ) {
        result.state = OperationState::Rejected {
            message: reason.into(),
            recovery: None,
        };
        self.result(result.clone());
        self.reply(peer, id, result);
    }
}
