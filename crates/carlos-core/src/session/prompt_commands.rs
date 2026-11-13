//! Submit and steer accepted prompts without borrowing a client widget.

use std::time::Instant;

use carlos_proto::submissions::{ClientOperation, RecoveryKind, SubmissionOutcome, SubmittedText};

use crate::backend::{BackendKind, PromptOrigin, PromptPriority};
use crate::protocol_params::params_turn_steer;

use super::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use super::claude::{ClaudePhase, NativeState};
use super::command_work::{
    BackendCall, BackendWork, CommandError, Finish, PreparedCommand, PromptSubmission,
};
use super::state::SessionState;
use crate::session::models::Role;

/// Keep an explicit send policy and originating operation without a live client draft.
#[derive(Clone)]
pub struct PromptContext {
    pub operation: Option<ClientOperation>,
    pub claude_queue: bool,
}

/// Keep the caller's explicit origin and history choice together.
#[derive(Clone, Copy)]
pub struct PromptOptions {
    pub record_history: bool,
    pub force_start: bool,
    pub origin: PromptOrigin,
}

/// Apply these immediate reports once, after the command returns.
#[derive(Default)]
pub struct TurnUpdate {
    pub changes: ClaudeUpdate,
    pub hint: Option<String>,
    pub clear_selection: bool,
    pub rewound: bool,
    pub submitted: Option<String>,
    pub name_saved: Option<String>,
}

impl SessionState {
    /// Prepare a submitted prompt without writing to or waiting on its backend.
    pub fn prepare_turn_text(
        &mut self,
        kind: BackendKind,
        text: String,
        options: PromptOptions,
        context: &PromptContext,
    ) -> Result<PreparedCommand, CommandError> {
        self.prepare_prompt(kind, text, options, context, false)
    }

    pub(super) fn prepare_prompt(
        &mut self,
        kind: BackendKind,
        text: String,
        options: PromptOptions,
        context: &PromptContext,
        edited: bool,
    ) -> Result<PreparedCommand, CommandError> {
        if self.command_idle().is_err() {
            return Err(CommandError::Busy {
                submitted: (options.origin == PromptOrigin::Human).then_some(SubmittedText {
                    text,
                    operation: context.operation.clone(),
                }),
            });
        }
        let mut prepared = PreparedCommand::default();
        if text.trim().is_empty()
            || self.reject_prompt(kind, &text, options, context, &mut prepared.update)
        {
            return Ok(prepared);
        }
        if kind == BackendKind::Claude && self.claude().phase() != ClaudePhase::Ready {
            self.enqueue_submitted_prompt(
                SubmittedText {
                    text,
                    operation: context.operation.clone(),
                },
                options.record_history,
                options.origin,
            );
            self.set_status("queued while Claude starts");
            return Ok(prepared);
        }
        let steer = !edited
            && self.active_turn_id().is_some()
            && !(kind.queues_instead_of_steer() && !options.force_start);
        if !steer && self.queue_behind_pending(kind, &text, options, context) {
            return Ok(prepared);
        }
        let submission = PromptSubmission {
            kind,
            text,
            options,
            context: context.clone(),
            steer,
            edited,
        };
        prepared.work = Some(self.prompt_work(submission));
        Ok(prepared)
    }

    fn prompt_work(&mut self, submission: PromptSubmission) -> BackendWork {
        let PromptSubmission {
            kind,
            ref text,
            options,
            ref context,
            steer,
            ..
        } = submission;
        self.dispatch_claude_edit(text, options.origin, context.operation.as_ref());
        let priority = if kind == BackendKind::Claude && context.claude_queue {
            PromptPriority::Queue
        } else if steer
            || kind == BackendKind::Claude
                && self.claude().activity().native_state() == NativeState::Running
        {
            PromptPriority::Steer
        } else {
            PromptPriority::Queue
        };
        let method = if steer { "turn/steer" } else { "turn/start" };
        let params = if steer {
            params_turn_steer(
                self.thread_id(),
                self.active_turn_id().as_ref().expect("steer turn"),
                text,
            )
        } else {
            self.new_turn_params(kind, text)
        };
        self.backend_work(
            kind,
            BackendCall::Prompt {
                method,
                params,
                priority,
            },
            Finish::Prompt(submission),
            steer,
        )
    }

    fn reject_prompt(
        &mut self,
        kind: BackendKind,
        text: &str,
        options: PromptOptions,
        context: &PromptContext,
        update: &mut TurnUpdate,
    ) -> bool {
        if kind == BackendKind::Codex && self.codex_scope().closed() {
            self.ralph_stop_after_failure();
            if options.origin == PromptOrigin::Human {
                update.changes.effects.push(ClaudeEffect::Submission(
                    SubmissionOutcome::Recovered {
                        draft: SubmittedText {
                            text: text.into(),
                            operation: context.operation.clone(),
                        },
                        kind: RecoveryKind::Prompt,
                    },
                ));
                self.append_message(
                    Role::System,
                    "Could not send message: Codex is closed; F5 reconnects; draft kept",
                );
            }
            update.hint = Some("Codex is closed; F5 reconnects; draft kept".into());
            return true;
        }
        if kind == BackendKind::Claude && self.claude().phase() == ClaudePhase::Closed {
            update.changes.items.reset_navigation |= self.discard_pending_prompt(text);
            update
                .changes
                .effects
                .push(ClaudeEffect::Submission(SubmissionOutcome::Recovered {
                    draft: SubmittedText {
                        text: text.into(),
                        operation: context.operation.clone(),
                    },
                    kind: RecoveryKind::Prompt,
                }));
            update.hint = Some("Claude is closed; restart Carlos to send this draft".into());
            self.append_message(
                Role::System,
                "Claude is closed; restart Carlos to send this draft",
            );
            return true;
        }
        false
    }

    fn queue_behind_pending(
        &mut self,
        kind: BackendKind,
        text: &str,
        options: PromptOptions,
        context: &PromptContext,
    ) -> bool {
        let now = Instant::now();
        if options.force_start
            || !kind.queues_follow_up_turns()
            || !self.has_ready_queued_turn_input(now)
        {
            return false;
        }
        self.promote_ready_continuation(now);
        self.enqueue_submitted_prompt(
            SubmittedText {
                text: text.into(),
                operation: context.operation.clone(),
            },
            options.record_history,
            options.origin,
        );
        self.set_status(if kind == BackendKind::Grok {
            "queued behind pending Grok turn"
        } else {
            "queued behind pending Claude turn"
        });
        true
    }
}

impl TurnUpdate {
    pub(super) fn merge(&mut self, other: ClaudeUpdate) {
        self.changes.items.history_replaced |= other.items.history_replaced;
        self.changes.items.reset_navigation |= other.items.reset_navigation;
        self.changes.effects.extend(other.effects);
        debug_assert!(other.action.is_none());
    }
}
