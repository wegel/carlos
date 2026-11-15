//! Refuse a second logical restore of a native address already claimed by an owner or launch.

use carlos_proto::ids::SessionId;
use carlos_proto::session::{Availability, Backend};
use carlos_proto::submissions::OperationState;

use super::Manager;

impl Manager {
    pub(super) fn reject_restore_conflict(&mut self, id: &SessionId) -> bool {
        let state = &self.sessions[id];
        if state.live.is_some() || state.launch.is_some() {
            return false;
        }
        let Some(native) = state.saved.native_thread.as_deref() else {
            return false;
        };
        let conflict = self.sessions.iter().any(|(other_id, other)| {
            other_id != id
                && other.saved.backend == state.saved.backend
                && (state.saved.backend == Backend::Codex || other.saved.cwd == state.saved.cwd)
                && (other.live.is_some() || other.launch.is_some())
                && other
                    .live
                    .as_ref()
                    .map(|live| live.native_thread.as_str())
                    .or(other.saved.native_thread.as_deref())
                    == Some(native)
        });
        if !conflict {
            return false;
        }
        let state = self.sessions.get_mut(id).expect("known restore target");
        state.info.availability = Availability::Unavailable;
        for mut result in std::mem::take(&mut state.create) {
            result.state = OperationState::Rejected {
                message: "native session already has another live or pending logical owner; no replacement was created".into(),
                recovery: None,
            };
            self.result(result);
        }
        true
    }
}
