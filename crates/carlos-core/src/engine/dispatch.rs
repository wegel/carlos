//! Admit ordinary commands in order and let exact-target controls bypass their native waits.

use std::time::Instant;

use carlos_proto::commands::SendPolicy;
use carlos_proto::submissions::{ClientOperation, OperationState, SubmittedText};

use super::mailbox::Envelope;
use super::owner::{failure, Owner};
use super::workers::Completion;
use super::{EngineCommand, EngineReply};
use crate::backend::{BackendKind, PromptOrigin};
use crate::session::entry_id::EntryId;
use crate::session::prompt_commands::{PromptContext, PromptOptions};

impl Owner {
    pub(super) fn admit_ordinary(&mut self) {
        for _ in 0..64 {
            let Ok(envelope) = self.channels.ordinary.try_recv() else {
                break;
            };
            if let Some(command) = self.admit(envelope) {
                self.ordinary.push_back(command);
            }
        }
    }

    fn admit(&mut self, envelope: Envelope) -> Option<EngineCommand> {
        let operation = match &envelope.command {
            EngineCommand::Send { operation, .. }
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
        Some(envelope.command)
    }

    pub(super) fn dispatch_controls(&mut self) {
        for _ in 0..64 {
            if self.workers.controls_busy >= 4 {
                break;
            }
            let Ok(envelope) = self.channels.control.try_recv() else {
                break;
            };
            let Some(command) = self.admit(envelope) else {
                continue;
            };
            if let EngineCommand::TaskStop {
                window,
                operation,
                task,
                expected_run,
            } = command
            {
                self.stop_task(window, operation, task, expected_run);
                continue;
            }
            self.dispatch_control(command);
        }
    }

    fn dispatch_control(&mut self, command: EngineCommand) {
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                let operation = match &command {
                    EngineCommand::Interrupt { operation, .. }
                    | EngineCommand::Approve { operation, .. } => operation,
                    _ => unreachable!("control lane"),
                };
                self.results.update(operation, rejected(error.to_string()));
                self.notice();
                return;
            }
        };
        let (operation, work) = match command {
            EngineCommand::Interrupt { operation, turn } => (
                operation,
                self.session
                    .prepare_engine_interrupt(self.client.as_ref(), &turn),
            ),
            EngineCommand::Approve {
                operation,
                card,
                answer,
            } => (
                operation,
                self.session.prepare_engine_answer(
                    self.client.as_ref(),
                    EntryId::from_wire(&card),
                    answer,
                ),
            ),
            _ => unreachable!("typed control lane"),
        };
        let prepared = work;
        match prepared {
            Ok(work) => {
                self.workers.controls_busy += 1;
                self.workers.controls.submit(move || Completion::Control {
                    operation,
                    completion: work.execute(&commands),
                });
            }
            Err(error) => self.results.update(&operation, rejected(error.to_string())),
        }
        self.notice();
    }

    pub(super) fn dispatch_ordinary(&mut self) {
        if matches!(
            self.session.event_availability(),
            carlos_proto::session::Availability::Starting
                | carlos_proto::session::Availability::Restoring
        ) {
            return;
        }
        if self.settings.is_some() {
            return;
        }
        if self.dispatch_front_queue() {
            return;
        }
        if self.workers.ordinary_busy || self.session.command_idle().is_err() {
            return;
        }
        if self.dispatch_pending_ralph_toggle() {
            return;
        }
        if self.dispatch_queued() {
            return;
        }
        if matches!(
            self.ordinary.front(),
            Some(EngineCommand::Send {
                policy: SendPolicy::Queue,
                ..
            })
        ) && self.client.kind() != BackendKind::Claude
            && self.session.active_turn_id().is_some()
            && !self.native.closed
        {
            return;
        }
        let Some(command) = self.ordinary.pop_front() else {
            return;
        };
        self.dispatch_command(command);
        if !self.ordinary.is_empty() && !self.workers.ordinary_busy {
            let _ = self.channels.wake.try_send(());
        }
    }

    fn dispatch_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Send {
                operation,
                text,
                policy,
            } => self.send(Some(operation), text, policy, PromptOrigin::Human, false),
            EngineCommand::SetRalph {
                operation,
                on,
                options,
            } => self.set_ralph(operation, on, options),
            EngineCommand::ToggleRalph { operation, options } => {
                self.toggle_ralph(operation, options)
            }
            EngineCommand::ToolLabelsSet {
                operation,
                expected,
                enabled,
                save_default,
            } => self.change_tool_labels(operation, expected, enabled, save_default),
            EngineCommand::ContextSet { operation, choice } => {
                self.save_context_default(operation, choice)
            }
            EngineCommand::SettingsSet {
                operation,
                expected,
                change,
                save_default,
            } => self.change_settings(operation, expected, change, save_default),
            EngineCommand::QueueChange {
                operation,
                expected,
                change,
            } => self.change_queue(operation, expected, change),
            command @ (EngineCommand::GoalSet { .. } | EngineCommand::GoalClear { .. }) => {
                self.change_goal(command)
            }
            command @ (EngineCommand::Rename { .. } | EngineCommand::Rewind { .. }) => {
                self.edit(command)
            }
            _ => unreachable!("typed ordinary lane"),
        }
    }

    fn dispatch_front_queue(&mut self) -> bool {
        if !self.session.command_allows_native_queue()
            || !matches!(
                self.ordinary.front(),
                Some(EngineCommand::QueueChange { .. })
            )
        {
            return false;
        }
        let Some(EngineCommand::QueueChange {
            operation,
            expected,
            change,
        }) = self.ordinary.pop_front()
        else {
            unreachable!("front queue change");
        };
        self.change_queue(operation, expected, change);
        if !self.ordinary.is_empty() {
            let _ = self.channels.wake.try_send(());
        }
        true
    }

    fn dispatch_queued(&mut self) -> bool {
        if self.session.active_turn_id().is_some() || self.native.closed {
            return false;
        }
        if !self.ordinary.is_empty()
            && self
                .session
                .ralph_runtime()
                .next_input_origin(Instant::now())
                == Some(PromptOrigin::Automatic)
        {
            return false;
        }
        let Some(turn) =
            self.session
                .take_dispatchable_prompt(self.client.kind(), true, Instant::now())
        else {
            return false;
        };
        self.send(
            turn.operation,
            turn.text,
            SendPolicy::Queue,
            turn.origin,
            true,
        );
        true
    }

    fn send(
        &mut self,
        operation: Option<ClientOperation>,
        text: String,
        policy: SendPolicy,
        origin: PromptOrigin,
        force_start: bool,
    ) {
        if self.native.closed || text.trim().is_empty() {
            if let Some(operation) = operation {
                self.results.recover(
                    SubmittedText {
                        operation: Some(operation),
                        text,
                    },
                    "backend closed or text was empty; nothing sent",
                    false,
                );
            }
            self.notice();
            return;
        }
        // Capture before claiming the core's mutation slot; a changing helper must not strand it.
        let commands = match self.client.capture_commands() {
            Ok(commands) => commands,
            Err(error) => {
                if let Some(operation) = operation {
                    self.results.recover(
                        SubmittedText {
                            operation: Some(operation),
                            text,
                        },
                        error.to_string(),
                        false,
                    );
                }
                self.notice();
                return;
            }
        };
        self.prepare_send(
            operation,
            text,
            policy,
            PromptOptions {
                record_history: origin == PromptOrigin::Human,
                force_start,
                origin,
            },
            commands,
        );
    }

    fn prepare_send(
        &mut self,
        operation: Option<ClientOperation>,
        text: String,
        policy: SendPolicy,
        options: PromptOptions,
        commands: crate::backend_commands::CommandHandle,
    ) {
        let context = PromptContext {
            operation: operation.clone(),
            claude_queue: policy == SendPolicy::Queue,
        };
        match self
            .session
            .prepare_turn_text(self.client.kind(), text, options, &context)
        {
            Ok(prepared) => {
                if let Some(operation) = &operation {
                    self.results.update(operation, OperationState::Queued);
                }
                self.turn_report(prepared.update);
                if let Some(work) = prepared.work {
                    self.workers.ordinary_busy = true;
                    self.workers.ordinary.submit(move || Completion::Ordinary {
                        operation,
                        completion: work.execute(&commands),
                    });
                }
            }
            Err(error) => {
                let message = error.to_string();
                if let Some(draft) = error.into_submitted() {
                    self.results.recover(draft, message, false);
                }
            }
        }
        self.notice();
    }
}

pub(super) fn rejected(message: impl Into<String>) -> OperationState {
    OperationState::Rejected {
        message: message.into(),
        recovery: None,
    }
}
