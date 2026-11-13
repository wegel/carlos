//! Submit and steer accepted prompts without borrowing a client widget.

use carlos_proto::submissions::SubmissionOutcome;

use std::time::{Duration, Instant};

use carlos_proto::submissions::{ClientOperation, RecoveryKind, SubmittedText};

use crate::backend::{BackendClient, BackendKind, PromptOrigin, PromptPriority};
use crate::protocol_params::{extract_result_object, params_turn_steer};

use super::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use super::claude::{ClaudePhase, NativeState};
use super::state::SessionState;
use crate::session::models::Role;

/// Borrow only the local choices needed by one accepted prompt command.
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
    /// Accept a prompt, preserving startup queues, closed drafts and steer priority.
    pub fn submit_turn_text(
        &mut self,
        client: &dyn BackendClient,
        text: String,
        options: PromptOptions,
        context: &PromptContext,
    ) -> TurnUpdate {
        let mut update = TurnUpdate::default();
        if text.trim().is_empty()
            || self.reject_prompt(client, &text, options, context, &mut update)
        {
            return update;
        }
        if client.kind() == BackendKind::Claude && self.claude().phase() != ClaudePhase::Ready {
            self.enqueue_submitted_prompt(
                SubmittedText {
                    text,
                    operation: context.operation.clone(),
                },
                options.record_history,
                options.origin,
            );
            self.set_status("queued while Claude starts");
            return update;
        }
        if !options.force_start
            && self.active_turn_id().is_some()
            && client.kind().queues_instead_of_steer()
        {
            self.start_prompt(
                client,
                text,
                PromptOptions {
                    force_start: true,
                    ..options
                },
                context,
                &mut update,
            );
        } else if let Some(turn) = self.active_turn_id().clone() {
            self.steer_prompt(client, &turn, &text, options, context, &mut update);
        } else {
            self.start_prompt(client, text, options, context, &mut update);
        }
        update
    }

    fn reject_prompt(
        &mut self,
        client: &dyn BackendClient,
        text: &str,
        options: PromptOptions,
        context: &PromptContext,
        update: &mut TurnUpdate,
    ) -> bool {
        if client.kind() == BackendKind::Codex && self.codex_scope().closed() {
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
        if client.kind() == BackendKind::Claude && self.claude().phase() == ClaudePhase::Closed {
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

    fn steer_prompt(
        &mut self,
        client: &dyn BackendClient,
        turn: &str,
        text: &str,
        options: PromptOptions,
        context: &PromptContext,
        update: &mut TurnUpdate,
    ) {
        let params = params_turn_steer(self.thread_id(), turn, text);
        let priority = if client.kind() == BackendKind::Claude && context.claude_queue {
            PromptPriority::Queue
        } else {
            PromptPriority::Steer
        };
        match client.submit_prompt(
            "turn/steer",
            params,
            options.origin,
            priority,
            Duration::from_secs(10),
        ) {
            Ok(response) => {
                if client.kind().echoes_local_user_prompt() {
                    self.echo_prompt(text, Role::User, options.record_history, update);
                }
                if client.kind() == BackendKind::Claude {
                    if let Ok(value) = serde_json::from_str(&response) {
                        update.merge(self.receive_claude_submission(&value));
                    }
                }
                self.set_status("sent steer");
            }
            Err(error) => self.set_status(error.to_string()),
        }
    }

    pub(super) fn start_prompt(
        &mut self,
        client: &dyn BackendClient,
        text: String,
        options: PromptOptions,
        context: &PromptContext,
        update: &mut TurnUpdate,
    ) -> bool {
        if client.kind() == BackendKind::Claude && self.claude().phase() != ClaudePhase::Ready {
            update.hint = Some("Claude is starting; answer any startup question first".into());
            return false;
        }
        if self.queue_behind_pending(client, &text, options, context) {
            return true;
        }
        self.dispatch_claude_edit(&text, options.origin);
        let priority = if client.kind() == BackendKind::Claude
            && self.claude().activity().native_state() == NativeState::Running
            && !context.claude_queue
        {
            PromptPriority::Steer
        } else {
            PromptPriority::Queue
        };
        let response = client
            .submit_prompt(
                "turn/start",
                self.new_turn_params(client, &text),
                options.origin,
                priority,
                Duration::from_secs(10),
            )
            .and_then(|response| extract_result_object(&response));
        match response {
            Ok(response) => {
                self.bind_claude_edit(&text, options.origin, &response);
                self.mark_prompt_sent(client, &text, options, update);
                if client.kind() == BackendKind::Claude {
                    update.merge(self.receive_claude_submission(&response));
                }
                true
            }
            Err(error) => {
                self.prompt_failed(&text, options, context, update, &error.to_string());
                false
            }
        }
    }

    fn prompt_failed(
        &mut self,
        text: &str,
        options: PromptOptions,
        context: &PromptContext,
        update: &mut TurnUpdate,
        error: &str,
    ) {
        if options.record_history {
            update.changes.items.reset_navigation |= self.discard_pending_prompt(text);
        }
        if self.reject_claude_edit_send(text) {
            self.fail_claude_edit(&mut update.changes, error);
        } else {
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
        }
        self.set_status(format!("send failed: {error}"));
        self.append_message(Role::System, format!("Could not send message: {error}"));
    }

    fn queue_behind_pending(
        &mut self,
        client: &dyn BackendClient,
        text: &str,
        options: PromptOptions,
        context: &PromptContext,
    ) -> bool {
        let now = Instant::now();
        if options.force_start
            || !client.kind().queues_follow_up_turns()
            || !self.has_ready_queued_turn_input(now)
        {
            return false;
        }
        self.promote_ready_continuation(now);
        self.queue_submitted_input(
            SubmittedText {
                text: text.into(),
                operation: context.operation.clone(),
            },
            options.record_history,
        );
        self.set_status(if client.kind() == BackendKind::Grok {
            "queued behind pending Grok turn"
        } else {
            "queued behind pending Claude turn"
        });
        true
    }

    fn mark_prompt_sent(
        &mut self,
        client: &dyn BackendClient,
        text: &str,
        options: PromptOptions,
        update: &mut TurnUpdate,
    ) {
        if client.kind().echoes_local_user_prompt() {
            let role = if client.kind() == BackendKind::Claude
                && options.origin == PromptOrigin::Automatic
            {
                Role::System
            } else {
                Role::User
            };
            self.echo_prompt(text, role, options.record_history, update);
            if self.active_turn_id().is_none() {
                self.mark_turn_started();
                self.set_active_turn_id(Some(
                    if client.kind() == BackendKind::Grok {
                        crate::grok_backend::GROK_PENDING_TURN_ID
                    } else {
                        "claude-turn-pending"
                    }
                    .into(),
                ));
            }
        }
        if client.kind() != BackendKind::Codex
            || !self.codex_scope().enabled()
            || self.codex_settings().legacy_overrides()
        {
            self.mark_runtime_settings_applied();
        }
        self.set_status("sent turn");
    }

    fn echo_prompt(&mut self, text: &str, role: Role, record: bool, update: &mut TurnUpdate) {
        let index = self.append_message(role, text);
        if record {
            update.changes.items.reset_navigation |=
                self.record_prompt_history(text, Some(self.messages()[index].id));
        }
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
