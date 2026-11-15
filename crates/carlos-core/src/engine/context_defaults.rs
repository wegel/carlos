//! Keep host-default disk work off the owner while preserving original read permits and private saves.

use carlos_proto::context_defaults::{ContextChoice, ContextDefaults};
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::mailbox::{full, Envelope};
use super::owner::{failure, Owner};
use super::workers::Completion;
use super::{EngineCommand, EngineReply};
use crate::context_defaults;

impl Owner {
    pub(super) fn defer_context_read(&mut self, envelope: Envelope) -> Option<Envelope> {
        if !matches!(&envelope.command, EngineCommand::ContextRead) {
            return Some(envelope);
        }
        if self.workers.actions_busy >= 2 {
            let _ = envelope.reply.try_send(EngineReply::Failure(full()));
            let _ = self.channels.notice.try_send(());
            return None;
        }
        let session = self.session.session_id().clone();
        let backend = self.client.kind();
        let attached = self.session.codex_connection().socket().is_some();
        let shell_override = self.session.context_shell_override();
        self.workers.actions_busy += 1;
        let pending = Box::new(envelope);
        self.workers
            .actions
            .submit(move || Completion::ContextRead {
                pending,
                result: context_defaults::read(session, backend, attached, shell_override)
                    .map_err(|error| error.to_string()),
            });
        None
    }

    pub(super) fn finish_context_read(
        &mut self,
        pending: Envelope,
        result: Result<ContextDefaults, String>,
    ) {
        self.workers.actions_busy -= 1;
        finish_read(pending, result);
        let _ = self.channels.notice.try_send(());
    }

    pub(super) fn save_context_default(
        &mut self,
        operation: ClientOperation,
        choice: ContextChoice,
    ) {
        let backend = self.client.kind();
        let attached = self.session.codex_connection().socket().is_some();
        let shell_override = self.session.context_shell_override();
        self.workers.ordinary_busy = true;
        self.workers
            .ordinary
            .submit(move || Completion::ContextSaved {
                operation,
                result: context_defaults::save(backend, attached, shell_override, choice)
                    .map_err(|error| error.to_string()),
            });
    }

    pub(super) fn finish_context_save(
        &mut self,
        operation: ClientOperation,
        result: Result<(), String>,
    ) {
        self.workers.ordinary_busy = false;
        let state = match result {
            Ok(()) => OperationState::Accepted {
                entry: None,
                notice: Some("Carlos saved the size for the next Codex or Claude launch.".into()),
            },
            Err(error) => rejected(error),
        };
        self.results.update(&operation, state);
        self.notice();
    }
}

fn finish_read(envelope: Envelope, result: Result<ContextDefaults, String>) {
    let reply = result
        .map(EngineReply::ContextDefaults)
        .unwrap_or_else(|error| EngineReply::Failure(failure(error)));
    let _ = envelope.reply.try_send(reply);
    // Dropping the original envelope now releases its mailbox permit even if the client left.
}

#[cfg(test)]
#[path = "context_defaults_tests.rs"]
mod tests;
