//! Own backend work without a SessionState borrow, then return a single guarded completion.

use std::fmt;
use std::time::Duration;

use carlos_proto::submissions::SubmittedText;
use serde_json::Value;

use super::command_guard::WorkGuard;
use super::prompt_commands::{PromptContext, PromptOptions, TurnUpdate};
use super::state::SessionState;
use crate::backend::{BackendClient, BackendKind, PromptPriority, RewindForkRequest};
use crate::protocol_params::extract_result_object;

/// An immediate session report plus optional work that a transport worker must execute.
#[derive(Default)]
pub struct PreparedCommand {
    pub update: TurnUpdate,
    pub work: Option<BackendWork>,
    pub status: PreparedStatus,
}

/// Distinguish native follow-up acknowledgement from definite validation/adoption failure.
#[derive(Default)]
pub enum PreparedStatus {
    #[default]
    Ready,
    Awaiting,
    Failed(String),
}

/// Immutable command data; execute consumes it so a caller cannot accidentally retry it.
pub struct BackendWork {
    pub(super) guard: WorkGuard,
    pub(super) call: BackendCall,
    pub(super) finish: Finish,
}

/// A reply and its original guard; only the owning session can settle it once.
pub struct BackendCompletion {
    pub(super) guard: WorkGuard,
    pub(super) finish: Finish,
    pub(super) result: Result<Value, String>,
}

pub(super) enum BackendCall {
    Prompt {
        method: &'static str,
        params: Value,
        priority: PromptPriority,
    },
    Fork {
        thread: String,
        request: RewindForkRequest,
        context: crate::backend_commands::ForkContext,
    },
    Rename(Value),
}

pub(super) enum Finish {
    Prompt(PromptSubmission),
    Rewind {
        submission: PromptSubmission,
        prefix: Option<Vec<super::models::EntryDraft>>,
    },
    Rename(String),
}

pub(super) struct PromptSubmission {
    pub(super) kind: BackendKind,
    pub(super) text: String,
    pub(super) options: PromptOptions,
    pub(super) context: PromptContext,
    pub(super) steer: bool,
    pub(super) edited: bool,
}

/// Busy commands were not sent; stale completions may have reached the backend and must not retry.
#[derive(Debug)]
pub enum CommandError {
    Busy { submitted: Option<SubmittedText> },
    Capacity { submitted: Option<SubmittedText> },
    Stale { submitted: Option<SubmittedText> },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Capacity { .. } => "native echo admission is full; nothing sent; draft kept",
            Self::Busy { .. } => "another backend command is pending; draft kept",
            Self::Stale { .. } => "backend command finished after its session changed; outcome uncertain; do not retry automatically",
        })
    }
}
impl std::error::Error for CommandError {}

impl BackendWork {
    /// Release a prepared slot through its normal completion path when no handle can be captured.
    pub(super) fn fail(self, message: String) -> BackendCompletion {
        BackendCompletion {
            guard: self.guard,
            finish: self.finish,
            result: Err(message),
        }
    }

    /// Wait for a backend without borrowing the session; the daemon runs this outside its owner loop.
    pub fn execute(self, client: &crate::backend_commands::CommandHandle) -> BackendCompletion {
        let permission =
            crate::dispatch_guard::DispatchGuard::from_flag(self.guard.dispatch_live.clone());
        match client.guarded(permission) {
            Ok(commands) => self.execute_via(&commands),
            Err(error) => BackendCompletion {
                guard: self.guard,
                finish: self.finish,
                result: Err(error.to_string()),
            },
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Drive fixed backend fixtures that do not capture a native command handle.
    pub fn execute_embedded(self, client: &dyn BackendClient) -> BackendCompletion {
        match client.capture_commands() {
            Ok(commands) => self.execute(&commands),
            Err(_) => self.execute_via(client),
        }
    }

    fn execute_via(self, client: &dyn BackendClient) -> BackendCompletion {
        let result = if !self
            .guard
            .dispatch_live
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            Err("backend command retired before dispatch; nothing sent".into())
        } else if !self.guard.matches_connection(client) {
            Err("backend kind or connection changed before command execution".into())
        } else {
            execute_call(self.call, &self.finish, client)
        };
        BackendCompletion {
            guard: self.guard,
            finish: self.finish,
            result,
        }
    }
}

impl SessionState {
    /// Settle only the matching connection/operation, then expose any prepared follow-up work.
    pub fn finish_backend_work(
        &mut self,
        completion: BackendCompletion,
    ) -> Result<PreparedCommand, CommandError> {
        if !self.accept_command_completion(&completion.guard) {
            // A written asynchronous fork may already have installed its own helper.
            // Its pre-registered operation continues through exact native edit facts;
            // this acknowledgement cannot restart it or overwrite a newer command.
            if completion.guard.belongs_to(self)
                && self.claude_edit_operation().is_some()
                && matches!(&completion.finish, Finish::Rewind { submission, .. }
                    if submission.kind == BackendKind::Claude
                    && self.claude_edit_operation() == submission.context.operation.as_ref())
                && completion
                    .result
                    .as_ref()
                    .is_ok_and(|value| value["result"]["claudeForkPending"] == true)
            {
                return Ok(PreparedCommand {
                    status: PreparedStatus::Awaiting,
                    ..Default::default()
                });
            }
            return Err(CommandError::Stale {
                submitted: completion.finish.submitted(),
            });
        }
        Ok(match completion.finish {
            Finish::Prompt(submission) => self.settle_prompt(submission, completion.result),
            Finish::Rewind { submission, prefix } => {
                self.settle_rewind(submission, prefix, completion.result)
            }
            Finish::Rename(name) => self.settle_name(name, completion.result),
        })
    }
}

impl Finish {
    pub(super) fn submitted(&self) -> Option<SubmittedText> {
        match self {
            Self::Prompt(prompt)
            | Self::Rewind {
                submission: prompt, ..
            } => (prompt.options.origin == crate::backend::PromptOrigin::Human).then_some(
                SubmittedText {
                    text: prompt.text.clone(),
                    operation: prompt.context.operation.clone(),
                },
            ),
            Self::Rename(_) => None,
        }
    }
}

impl CommandError {
    /// Return caller-owned submitted text even when a wait could not be prepared or became stale.
    pub fn into_submitted(self) -> Option<SubmittedText> {
        match self {
            Self::Busy { submitted } | Self::Stale { submitted } | Self::Capacity { submitted } => {
                submitted
            }
        }
    }
}

fn execute_call(
    call: BackendCall,
    finish: &Finish,
    client: &dyn BackendClient,
) -> Result<Value, String> {
    let result = match call {
        BackendCall::Prompt {
            method,
            params,
            priority,
        } => {
            let Finish::Prompt(submission) = finish else {
                unreachable!("prompt completion");
            };
            client.submit_prompt(
                method,
                params,
                submission.options.origin,
                priority,
                Duration::from_secs(10),
            )
        }
        BackendCall::Fork {
            thread,
            request,
            context,
        } => client.fork_with_context(&thread, request, Duration::from_secs(20), context),
        BackendCall::Rename(params) => {
            client.call("thread/name/set", params, Duration::from_secs(10))
        }
    };
    result
        .and_then(|response| extract_result_object(&response))
        .map_err(|error| error.to_string())
}
