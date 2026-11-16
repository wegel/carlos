//! Apply each worker completion once and retain the originating client's private result.

use carlos_proto::submissions::OperationState;

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::session::control_work::ControlFailure;

impl Owner {
    pub(super) fn drain_completions(&mut self) {
        while let Ok(completion) = self.workers.completed.try_recv() {
            self.finish_completion(completion);
            self.drain_control_events();
            self.notice();
        }
    }

    fn finish_completion(&mut self, completion: Completion) {
        match completion {
            Completion::History {
                pending,
                target,
                cursor,
                result,
            } => {
                self.finish_history_read(*pending, target, cursor, result);
            }
            Completion::ClaudeReopened { operation, result } => {
                self.finish_claude_reopen(operation, result)
            }
            Completion::ContextRead { pending, result } => {
                self.finish_context_read(*pending, result)
            }
            Completion::ContextSaved { operation, result } => {
                self.finish_context_save(operation, result)
            }
            Completion::ToolLabelsSaved { operation, error } => {
                self.finish_tool_labels_save(operation, error)
            }
            Completion::BackendSettings { operation, result } => {
                self.finish_backend_settings(operation, result);
            }
            Completion::SettingsSaved { operation, error } => {
                self.finish_settings_save(operation, error);
            }
            Completion::TaskStopDispatch { operation, result } => {
                self.finish_task_stop_dispatch(operation, result);
            }
            Completion::Ordinary {
                operation,
                completion,
            } => {
                self.finish_ordinary(operation, completion);
            }
            Completion::Control {
                operation,
                completion,
            } => {
                self.finish_control(operation, completion);
            }
            Completion::InternalControl(completion) => self.finish_internal_control(completion),
            Completion::Ralph {
                operation,
                result,
                target,
            } => {
                self.finish_ralph_completion(target, operation, result);
            }
            Completion::Action {
                target,
                result,
                kind,
            } => {
                self.finish_action_completion(target, kind, result);
            }
        }
    }
    fn finish_internal_control(
        &mut self,
        completion: crate::session::control_work::ControlCompletion,
    ) {
        self.workers.actions_busy -= 1;
        if let Err(error) = self
            .session
            .finish_engine_control(self.client.as_ref(), completion)
        {
            self.session.set_status(error.to_string());
        }
    }

    fn finish_control(
        &mut self,
        operation: carlos_proto::submissions::ClientOperation,
        completion: crate::session::control_work::ControlCompletion,
    ) {
        self.workers.controls_busy -= 1;
        let state = match self
            .session
            .finish_engine_control(self.client.as_ref(), completion)
        {
            Ok(()) => OperationState::Accepted {
                entry: None,
                notice: None,
            },
            Err(ControlFailure::Invalid(message)) => rejected(message),
            Err(error) => OperationState::Uncertain {
                message: error.to_string(),
                recovery: None,
            },
        };
        self.results.update(&operation, state);
    }

    fn finish_ralph_completion(
        &mut self,
        target: super::target::Target,
        operation: Option<carlos_proto::submissions::ClientOperation>,
        result: Result<crate::session::ralph::RalphConfig, String>,
    ) {
        self.workers.ordinary_busy = false;
        if target.matches(&self.session, self.client.kind()) {
            self.finish_ralph(operation, result);
        } else if let Some(operation) = operation {
            self.results.update(
                &operation,
                rejected("session changed while reading Ralph prompt; nothing enabled"),
            );
        }
    }

    fn finish_action_completion(
        &mut self,
        target: super::target::Target,
        kind: super::actions::ActionKind,
        result: Result<bool, super::actions::ActionFailure>,
    ) {
        self.workers.actions_busy -= 1;
        if matches!(&kind, super::actions::ActionKind::QuotaRead(_)) {
            self.finish_action(kind, result);
        } else if let (
            super::actions::ActionKind::ContextRead(token),
            Err(super::actions::ActionFailure::Retired),
        ) = (&kind, &result)
        {
            if target.same_helper(&self.session, self.client.kind()) {
                self.session
                    .fail_claude_context_request(*token, std::time::Instant::now());
            }
        } else if target.matches(&self.session, self.client.kind()) {
            self.finish_action(kind, result);
        }
    }

    pub(super) fn finish_ordinary(
        &mut self,
        operation: Option<carlos_proto::submissions::ClientOperation>,
        completion: crate::session::command_work::BackendCompletion,
    ) {
        self.workers.ordinary_busy = false;
        let error = completion.result.as_ref().err().cloned();
        let recovery = completion.finish.submitted();
        match self.session.finish_backend_work(completion) {
            Ok(prepared) => {
                self.settle_prepared(operation, prepared, recovery, error, true);
            }
            Err(error) => {
                self.edit_failed(operation.as_ref(), recovery, error.to_string(), true);
            }
        }
    }
}
