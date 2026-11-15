//! Keep rename and fork/send chains on the ordered lane with private checked completion.

use carlos_proto::submissions::{ClientOperation, OperationState, SubmittedText};

use super::owner::Owner;
use super::workers::Completion;
use super::EngineCommand;
use crate::session::command_work::{PreparedCommand, PreparedStatus};
use crate::session::entry_id::EntryId;
use crate::session::prompt_commands::PromptContext;

impl Owner {
    pub(super) fn edit(&mut self, command: EngineCommand) {
        let (operation, recovery) = match &command {
            EngineCommand::Rename { operation, .. } => (operation.clone(), None),
            EngineCommand::Rewind {
                operation, text, ..
            } => (
                operation.clone(),
                Some(SubmittedText {
                    operation: Some(operation.clone()),
                    text: text.clone(),
                }),
            ),
            _ => unreachable!("ordered edit command"),
        };
        // Capture before preparing so an unavailable handle cannot strand a prepared slot.
        let commands = match self.client.capture_commands() {
            Ok(commands) if !self.native.closed => commands,
            result => {
                let message = result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "backend closed; nothing sent".into());
                self.edit_failed(Some(&operation), recovery, message, false);
                return;
            }
        };
        let prepared = match command {
            EngineCommand::Rename { name, .. } => {
                self.session.prepare_rename(self.client.kind(), &name)
            }
            EngineCommand::Rewind { entry, text, .. } => self.session.prepare_rewind_text(
                self.client.kind(),
                EntryId::from_wire(&entry),
                text,
                &PromptContext {
                    operation: Some(operation.clone()),
                    claude_queue: false,
                },
                false,
            ),
            _ => unreachable!("ordered edit command"),
        };
        match prepared {
            Ok(mut prepared) => {
                if let Some(work) = prepared.work.take() {
                    self.turn_report(prepared.update);
                    self.workers.ordinary_busy = true;
                    self.workers.ordinary.submit(move || Completion::Ordinary {
                        operation: Some(operation),
                        completion: work.execute(&commands),
                    });
                } else {
                    self.settle_prepared(Some(operation), prepared, recovery, None, false);
                }
            }
            Err(error) => self.edit_failed(Some(&operation), recovery, error.to_string(), false),
        }
        self.notice();
    }

    pub(super) fn settle_prepared(
        &mut self,
        operation: Option<ClientOperation>,
        prepared: PreparedCommand,
        recovery: Option<SubmittedText>,
        native_error: Option<String>,
        after_native: bool,
    ) {
        self.turn_report(prepared.update);
        if let Some(message) = native_error {
            self.edit_failed(operation.as_ref(), recovery, message, true);
        } else if let PreparedStatus::Failed(message) = prepared.status {
            self.edit_failed(operation.as_ref(), recovery, message, after_native);
        } else if let Some(work) = prepared.work {
            match self.client.capture_commands() {
                Ok(commands) => {
                    self.workers.ordinary_busy = true;
                    self.workers.ordinary.submit(move || Completion::Ordinary {
                        operation,
                        completion: work.execute(&commands),
                    });
                }
                Err(error) => self.finish_ordinary(operation, work.fail(error.to_string())),
            }
        } else if !matches!(prepared.status, PreparedStatus::Awaiting) {
            if let Some(operation) = operation {
                self.results.update(
                    &operation,
                    OperationState::Accepted {
                        entry: None,
                        notice: None,
                    },
                );
            }
        }
        self.notice();
    }

    pub(super) fn edit_failed(
        &mut self,
        operation: Option<&ClientOperation>,
        recovery: Option<SubmittedText>,
        message: String,
        uncertain: bool,
    ) {
        if let Some(operation) = operation {
            let state = if uncertain {
                OperationState::Uncertain { message, recovery }
            } else {
                OperationState::Rejected { message, recovery }
            };
            self.results.update(operation, state);
        }
        self.notice();
    }
}
