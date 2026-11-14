//! Apply each worker completion once and retain the originating client's private result.

use carlos_proto::submissions::OperationState;

use super::dispatch::rejected;
use super::owner::Owner;
use super::workers::Completion;
use crate::session::control_work::ControlFailure;

impl Owner {
    pub(super) fn drain_completions(&mut self) {
        while let Ok(completion) = self.workers.completed.try_recv() {
            match completion {
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
                    self.workers.controls_busy -= 1;
                    let state = match self
                        .session
                        .finish_engine_control(self.client.as_ref(), completion)
                    {
                        Ok(()) => OperationState::Accepted { entry: None },
                        Err(ControlFailure::Invalid(message)) => rejected(message),
                        Err(error) => OperationState::Uncertain {
                            message: error.to_string(),
                            recovery: None,
                        },
                    };
                    self.results.update(&operation, state);
                }
                Completion::InternalControl(completion) => {
                    self.workers.actions_busy -= 1;
                    if let Err(error) = self
                        .session
                        .finish_engine_control(self.client.as_ref(), completion)
                    {
                        self.session.set_status(error.to_string());
                    }
                }
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
            self.notice();
        }
    }
    fn finish_ralph_completion(
        &mut self,
        target: super::target::Target,
        operation: carlos_proto::submissions::ClientOperation,
        result: Result<crate::session::ralph::RalphConfig, String>,
    ) {
        self.workers.ordinary_busy = false;
        if target.matches(&self.session, self.client.kind()) {
            self.finish_ralph(operation, result);
        } else {
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
        if let (
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

    fn finish_ordinary(
        &mut self,
        operation: Option<carlos_proto::submissions::ClientOperation>,
        completion: crate::session::command_work::BackendCompletion,
    ) {
        self.workers.ordinary_busy = false;
        let error = completion.result.as_ref().err().cloned();
        match self.session.finish_backend_work(completion) {
            Ok(prepared) => {
                if let Some(operation) = &operation {
                    self.results.update(
                        operation,
                        error.map_or(OperationState::Accepted { entry: None }, |message| {
                            OperationState::Uncertain {
                                message,
                                recovery: None,
                            }
                        }),
                    );
                }
                self.turn_report(prepared.update);
                assert!(
                    prepared.work.is_none(),
                    "basic send cannot start a second ordinary job"
                );
            }
            Err(error) => {
                let message = error.to_string();
                if let Some(draft) = error.into_submitted() {
                    self.results.recover(draft, message, true);
                }
            }
        }
    }
}
