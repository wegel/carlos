//! Save accepted session names and report local composer effects.

use crate::backend::BackendKind;
use crate::protocol_params::params_thread_set_name;

use super::command_work::{BackendCall, CommandError, Finish, PreparedCommand};
use super::prompt_commands::TurnUpdate;
use super::state::SessionState;
use crate::session::models::Role;
use serde_json::Value;

impl SessionState {
    /// Reject unsupported or unsafe names before sending the existing native command.
    pub fn prepare_rename(
        &mut self,
        kind: BackendKind,
        name: &str,
    ) -> Result<PreparedCommand, CommandError> {
        self.command_idle()?;
        let mut update = TurnUpdate::default();
        let validation = if !matches!(kind, BackendKind::Codex | BackendKind::Claude) {
            Some("this backend does not support renaming sessions")
        } else if name.chars().any(char::is_control) {
            Some("use a single-line session name without control characters")
        } else {
            None
        };
        if let Some(error) = validation {
            self.name_failed(error, false, &mut update);
            return Ok(PreparedCommand { update, work: None });
        }
        let call = BackendCall::Rename(params_thread_set_name(self.thread_id(), name));
        let work = self.backend_work(kind, call, Finish::Rename(name.to_owned()), false);
        Ok(PreparedCommand {
            update,
            work: Some(work),
        })
    }

    pub(super) fn settle_name(
        &mut self,
        name: String,
        result: Result<Value, String>,
    ) -> PreparedCommand {
        let mut update = TurnUpdate::default();
        match result {
            Ok(_) => {
                self.observe_name(Some(&name));
                update.name_saved = Some(name.to_owned());
                self.set_status("session renamed");
            }
            Err(error) => self.name_failed(&error.to_string(), true, &mut update),
        }
        PreparedCommand { update, work: None }
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
