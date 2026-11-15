//! Admit private operations once and capture reopen scope before ordinary FIFO delays.

use super::mailbox::Envelope;
use super::owner::{failure, Owner};
use super::{EngineCommand, EngineReply};

impl Owner {
    pub(super) fn admit(&mut self, mut envelope: Envelope) -> Option<EngineCommand> {
        let operation = match &envelope.command {
            EngineCommand::Reopen { operation, .. }
            | EngineCommand::Send { operation, .. }
            | EngineCommand::Rename { operation, .. }
            | EngineCommand::Rewind { operation, .. }
            | EngineCommand::SetRalph { operation, .. }
            | EngineCommand::ToggleRalph { operation, .. }
            | EngineCommand::GoalSet { operation, .. }
            | EngineCommand::GoalClear { operation, .. }
            | EngineCommand::QueueChange { operation, .. }
            | EngineCommand::ContextSet { operation, .. }
            | EngineCommand::SettingsSet { operation, .. }
            | EngineCommand::ToolLabelsSet { operation, .. }
            | EngineCommand::TaskStop { operation, .. }
            | EngineCommand::Approve { operation, .. }
            | EngineCommand::Interrupt { operation, .. } => operation,
            EngineCommand::Reconnect { operation } => operation,
            _ => {
                let _ = envelope.reply.try_send(EngineReply::Failure(failure(
                    "read entered a mutation lane",
                )));
                return None;
            }
        };
        if self.results.contains(operation) {
            let _ = envelope.reply.try_send(EngineReply::Failure(failure(
                "operation already admitted; query its retained result",
            )));
            return None;
        }
        let result = self.results.admit(
            self.session.session_id(),
            operation.clone(),
            envelope.permit,
        );
        let _ = envelope.reply.try_send(EngineReply::Operation(result));
        if self.admit_claude_reopen(&mut envelope.command) {
            return None;
        }
        Some(envelope.command)
    }
}
