//! Keep a rename draft local and change the saved name only after a private acknowledgement.

use anyhow::{bail, Result};
use carlos_proto::commands::Command;
use carlos_proto::session::Backend;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::transport::Transport;
use super::Device;

pub(super) struct Rename {
    pub operation: ClientOperation,
    command: String,
    retained: bool,
}

impl Device {
    pub(super) fn rename_command(
        &mut self,
        link: Option<&mut Transport>,
        text: &str,
    ) -> Result<bool> {
        let Some(rest) = text.trim().strip_prefix("/rename") else {
            return Ok(false);
        };
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return Ok(false);
        }
        if self.rename.is_some() {
            bail!("A session rename is pending; draft kept");
        }
        let link =
            link.ok_or_else(|| anyhow::anyhow!("Disconnected; rename draft kept; F5 reconnects"))?;
        if !self.client.session_is_ready(link.connection, &self.session) {
            bail!("Waiting for current ready session facts; rename draft kept");
        }
        if rest.trim().is_empty()
            && matches!(self.task_backend(), Some(Backend::Codex | Backend::Claude))
        {
            let name = self
                .client
                .session(&self.session)
                .and_then(|session| session.info().name.as_deref())
                .unwrap_or("");
            self.set_composer(&format!("/rename {name}"));
            return Ok(true);
        }
        let session = self.session.clone();
        let name = rest.trim().to_owned();
        let mut submitted = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            submitted = Some(operation.clone());
            Command::Rename {
                session,
                operation,
                name,
            }
        })?;
        self.rename = submitted.map(|operation| Rename {
            operation,
            command: text.into(),
            retained: false,
        });
        self.view.status_hint = None;
        link.send(outgoing)?;
        self.set_composer("");
        Ok(true)
    }

    pub(super) fn apply_rename_operation(&mut self, id: &ClientOperation) -> bool {
        if !self
            .rename
            .as_ref()
            .is_some_and(|rename| &rename.operation == id)
        {
            return false;
        }
        if self.rename.as_ref().is_some_and(|rename| rename.retained) {
            return true;
        }
        let Some(record) = self.client.operations().get(id) else {
            return true;
        };
        let failure = match &record.state {
            OperationState::Pending | OperationState::Queued => return true,
            OperationState::Accepted { .. } => None,
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => Some(message.clone()),
        };
        let mut rename = self.rename.take().expect("originating rename");
        if let Some(failure) = failure {
            self.view.status_hint = Some(format!("Could not rename session: {failure}"));
            if self.view.input.is_empty() {
                self.set_composer(&rename.command);
            } else {
                self.view.status_hint = Some(format!("Could not rename session: {failure}; F5 loads the saved rename draft when the composer is empty"));
                rename.retained = true;
                self.rename = Some(rename);
                return true;
            }
        }
        self.client.forget_operation(id);
        true
    }

    pub(super) fn recover_rename(&mut self) -> bool {
        if !self.rename.as_ref().is_some_and(|rename| rename.retained) {
            return false;
        }
        if !self.view.input.is_empty() {
            self.view.status_hint =
                Some("Copy or clear the current draft before F5 loads the saved rename".into());
            return true;
        }
        let rename = self.rename.take().expect("retained rename");
        self.set_composer(&rename.command);
        self.client.forget_operation(&rename.operation);
        true
    }
}
