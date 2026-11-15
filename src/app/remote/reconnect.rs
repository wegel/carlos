//! Start native Codex recovery only after this device accepts a current closed-session snapshot.

use anyhow::Result;
use carlos_proto::commands::Command;
use carlos_proto::session::{Availability, Backend};
use carlos_proto::submissions::OperationState;

use super::transport::Transport;
use super::Device;

impl Device {
    pub(super) fn native_reconnect(&mut self, link: &mut Transport) -> Result<()> {
        if self.reconnect.is_some()
            || !self
                .client
                .session_has_current_snapshot(link.connection, &self.session)
            || self.client.session(&self.session).is_none_or(|replica| {
                replica.info().backend != Backend::Codex
                    || replica.info().availability != Availability::Closed
            })
        {
            return Ok(());
        }
        let session = self.session.clone();
        let mut submitted = None;
        let outgoing = self.client.submit(link.connection, |operation| {
            submitted = Some(operation.clone());
            Command::Reconnect { session, operation }
        })?;
        self.reconnect = submitted;
        self.view.status_hint = None;
        link.send(outgoing)
    }

    pub(super) fn drive_reconnect(&mut self) -> bool {
        let Some(operation) = self.reconnect.as_ref() else {
            return false;
        };
        let Some(result) = self.client.operations().get(operation) else {
            return false;
        };
        match &result.state {
            OperationState::Pending | OperationState::Queued => return false,
            OperationState::Accepted { .. } => {}
            OperationState::Rejected { message, .. }
            | OperationState::Uncertain { message, .. } => {
                self.view.status_hint = Some(message.clone());
            }
        }
        let operation = self.reconnect.take().expect("matched reconnect result");
        self.client.forget_operation(&operation);
        true
    }
}
