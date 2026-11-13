//! Save accepted session names and report local composer effects.

use std::time::Duration;

use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::{extract_result_object, params_thread_set_name};

use super::claude::receive_update::ClaudeEffect;
use super::prompt_commands::TurnUpdate;
use super::state::SessionState;
use crate::session::models::Role;

impl SessionState {
    /// Reject unsupported or unsafe names before sending the existing native command.
    pub fn rename_session(&mut self, client: &dyn BackendClient, name: &str) -> TurnUpdate {
        let mut update = TurnUpdate::default();
        let validation = if !matches!(client.kind(), BackendKind::Codex | BackendKind::Claude) {
            Some("this backend does not support renaming sessions")
        } else if name.chars().any(char::is_control) {
            Some("use a single-line session name without control characters")
        } else {
            None
        };
        if let Some(error) = validation {
            self.name_failed(error, false, &mut update);
            return update;
        }
        let result = client
            .call(
                "thread/name/set",
                params_thread_set_name(self.thread_id(), name),
                Duration::from_secs(10),
            )
            .and_then(|response| extract_result_object(&response));
        match result {
            Ok(_) => {
                self.observe_name(Some(name));
                update.changes.effects.push(ClaudeEffect::ClearInput);
                self.set_status("session renamed");
            }
            Err(error) => self.name_failed(&error.to_string(), true, &mut update),
        }
        update
    }

    fn name_failed(&mut self, error: &str, backend: bool, update: &mut TurnUpdate) {
        let message = format!("Could not rename session: {error}");
        if backend {
            self.set_status(message.clone());
        } else {
            update.hint = Some(message.clone());
        }
        self.append_message(Role::System, message);
    }
}
