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
    Stale { submitted: Option<SubmittedText> },
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy { .. } => "another backend command is pending; draft kept",
            Self::Stale { .. } => "backend command finished after its session changed; outcome uncertain; do not retry automatically",
        })
    }
}
impl std::error::Error for CommandError {}

impl BackendWork {
    /// Wait for a backend without borrowing the session; the daemon runs this outside its owner loop.
    pub fn execute(self, client: &dyn BackendClient) -> BackendCompletion {
        let result = if client.kind() != self.guard.kind {
            Err("backend kind changed before command execution".into())
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
    fn submitted(self) -> Option<SubmittedText> {
        match self {
            Self::Prompt(prompt)
            | Self::Rewind {
                submission: prompt, ..
            } => (prompt.options.origin == crate::backend::PromptOrigin::Human).then_some(
                SubmittedText {
                    text: prompt.text,
                    operation: prompt.context.operation,
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
            Self::Busy { submitted } | Self::Stale { submitted } => submitted,
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
        BackendCall::Fork { thread, request } => {
            client.fork_from_rewind(&thread, request, Duration::from_secs(20))
        }
        BackendCall::Rename(params) => {
            client.call("thread/name/set", params, Duration::from_secs(10))
        }
    };
    result
        .and_then(|response| extract_result_object(&response))
        .map_err(|error| error.to_string())
}
