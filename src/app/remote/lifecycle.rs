//! Keep explicit stop/reopen outcomes private without replacing a newer composer draft.

use anyhow::{bail, Context, Result};
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::{Device, Transport};
use crate::app::session_cli::Action;

pub(super) struct Intent {
    operation: ClientOperation,
    action: Action,
}

impl Device {
    pub(super) fn lifecycle_command(
        &mut self,
        link: Option<&mut Transport>,
        text: &str,
    ) -> Result<bool> {
        let mut words = text.split_whitespace();
        if words.next() != Some("/session") {
            return Ok(false);
        }
        let action = Action::parse(words.next().unwrap_or(""))?;
        if words.next().is_some() {
            bail!("Use /session stop or /session reopen for this session");
        }
        if self.lifecycle.is_some() {
            bail!("A session action is pending; draft kept");
        }
        let link = link.context("Disconnected; session command kept; F5 reconnects")?;
        let session = self.session.clone();
        let mut intent = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            intent = Some(Intent {
                operation: operation.clone(),
                action,
            });
            action.command(session, operation)
        })?;
        self.lifecycle = intent;
        link.send(outgoing)?;
        self.set_composer("");
        self.view.status_hint = Some(
            match action {
                Action::Stop => "Stopping session; saved conversation and drafts will be kept",
                Action::Reopen => "Reopening saved session; drafts will be kept",
            }
            .into(),
        );
        Ok(true)
    }

    pub(super) fn apply_lifecycle_operation(&mut self, operation: &ClientOperation) -> bool {
        let Some(intent) = self
            .lifecycle
            .as_ref()
            .filter(|intent| &intent.operation == operation)
        else {
            return false;
        };
        let Some(record) = self.client.operations().get(operation) else {
            return true;
        };
        let message = match &record.state {
            OperationState::Pending | OperationState::Queued => return true,
            OperationState::Accepted { .. } => match intent.action {
                Action::Stop => "Session stopped; /session reopen restores it; draft kept".into(),
                Action::Reopen => "Session reopened; press Enter to send your draft".into(),
            },
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => message.clone(),
        };
        self.view.status_hint = Some(message);
        self.lifecycle = None;
        self.client.forget_operation(operation);
        true
    }
}
