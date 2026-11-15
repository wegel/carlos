//! Save accepted session names and report command failures without client editor state.

use crate::backend::BackendKind;
use crate::protocol_params::params_thread_set_name;

use super::command_work::{BackendCall, CommandError, Finish, PreparedCommand, PreparedStatus};
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
        let update = TurnUpdate::default();
        let validation = if !matches!(kind, BackendKind::Codex | BackendKind::Claude) {
            Some("this backend does not support renaming sessions")
        } else if name.chars().any(char::is_control) {
            Some("use a single-line session name without control characters")
        } else {
            None
        };
        if let Some(error) = validation {
            self.name_failed(error, false);
            return Ok(PreparedCommand {
                update,
                status: PreparedStatus::Failed(error.into()),
                ..Default::default()
            });
        }
        let call = BackendCall::Rename(params_thread_set_name(self.thread_id(), name));
        let work = self.backend_work(kind, call, Finish::Rename(name.to_owned()), false);
        Ok(PreparedCommand {
            update,
            work: Some(work),
            ..Default::default()
        })
    }

    pub(super) fn settle_name(
        &mut self,
        name: String,
        result: Result<Value, String>,
    ) -> PreparedCommand {
        let update = TurnUpdate::default();
        match result {
            Ok(_) => {
                self.observe_name(Some(&name));
                self.set_status("session renamed");
            }
            Err(error) => self.name_failed(&error.to_string(), true),
        }
        PreparedCommand {
            update,
            ..Default::default()
        }
    }

    fn name_failed(&mut self, error: &str, backend: bool) {
        let message = format!("Could not rename session: {error}");
        if backend {
            self.set_status(message.clone());
        }
        self.append_message(Role::System, message);
    }
}
