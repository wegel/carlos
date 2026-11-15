//! Keep explicit Claude recovery on the existing owner and ordinary worker.

use std::sync::atomic::Ordering;

use carlos_proto::session::Availability;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::backend::BackendKind;
use crate::dispatch_guard::DispatchGuard;

impl Owner {
    pub(super) fn admit_claude_reopen(&mut self, command: &mut super::EngineCommand) -> bool {
        let super::EngineCommand::Reopen {
            operation,
            generation,
        } = command
        else {
            return false;
        };
        let accepted = self.session.command_connection(BackendKind::Claude).1;
        if self.client.kind() == BackendKind::Claude
            && (self.reopening
                || self.session.event_availability() != Availability::Closed
                || self.client.connection_generation() != Some(accepted))
        {
            self.results.update(operation, attached());
            self.notice();
            return true;
        }
        *generation = Some(accepted);
        false
    }

    pub(super) fn reopen_claude(&mut self, operation: ClientOperation, generation: u64) {
        if self.client.kind() != BackendKind::Claude {
            self.results.update(
                &operation,
                rejected("explicit helper reopen is supported only for Claude"),
            );
            return;
        }
        if self.client.connection_generation() != Some(generation)
            || self.session.event_availability() != Availability::Closed
        {
            self.results.update(&operation, attached());
            return;
        }
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                self.results.update(&operation, rejected(error.to_string()));
                return;
            }
        };
        let stop = self.channels.stop.clone();
        let permission = DispatchGuard::new(move || !stop.load(Ordering::SeqCst));
        let commands = match commands.guarded(permission) {
            Ok(commands) => commands,
            Err(error) => {
                self.results.update(&operation, rejected(error.to_string()));
                return;
            }
        };
        let runtime = self.session.runtime_settings();
        let (model, effort) = self.session.claude_prompt_choices(
            runtime.current_model().clone(),
            runtime.current_effort().clone(),
        );
        let thread = self.session.thread_id().clone();
        self.actions.retire();
        self.reopening = true;
        self.workers.ordinary_busy = true;
        self.workers
            .ordinary
            .submit(move || Completion::ClaudeReopened {
                operation,
                result: commands
                    .reopen_session(&thread, model, effort)
                    .map_err(|error| error.to_string()),
            });
    }

    pub(super) fn finish_claude_reopen(
        &mut self,
        operation: ClientOperation,
        result: Result<u64, String>,
    ) {
        self.workers.ordinary_busy = false;
        self.reopening = false;
        let state = match result {
            Ok(_) => OperationState::Accepted {
                entry: None,
                notice: Some("Claude reopen started; unsent text was not resent".into()),
            },
            Err(message) => rejected(message),
        };
        self.results.update(&operation, state);
        self.finish_claude_ready();
    }

    pub(super) fn finish_claude_ready(&mut self) {
        if self.client.kind() == BackendKind::Claude
            && self.native.closed
            && self.session.event_availability() == Availability::Ready
            && self.client.connection_generation()
                == Some(self.session.command_connection(BackendKind::Claude).1)
        {
            self.native.closed = false;
        }
    }
}

pub(super) fn attached() -> OperationState {
    OperationState::Accepted {
        entry: None,
        notice: Some("Attached current Claude owner; no additional helper started".into()),
    }
}
