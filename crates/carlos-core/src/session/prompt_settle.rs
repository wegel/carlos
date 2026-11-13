//! Apply accepted prompt replies after backend work has released the session owner.

use carlos_proto::submissions::{RecoveryKind, SubmissionOutcome, SubmittedText};
use serde_json::Value;

use super::claude::receive_update::ClaudeEffect;
use super::command_work::{PreparedCommand, PromptSubmission};
use super::models::Role;
use super::prompt_commands::{PromptContext, PromptOptions, TurnUpdate};
use super::state::SessionState;
use crate::backend::{BackendKind, PromptOrigin};

impl SessionState {
    pub(super) fn settle_prompt(
        &mut self,
        submission: PromptSubmission,
        result: Result<Value, String>,
    ) -> PreparedCommand {
        let mut prepared = PreparedCommand::default();
        match result {
            Ok(response) => self.apply_prompt_success(&submission, &response, &mut prepared.update),
            Err(error) if submission.steer => self.set_status(error),
            Err(error) => self.prompt_failed(
                &submission.text,
                submission.options,
                &submission.context,
                &mut prepared.update,
                &error,
            ),
        }
        prepared
    }

    fn apply_prompt_success(
        &mut self,
        submission: &PromptSubmission,
        response: &Value,
        update: &mut TurnUpdate,
    ) {
        let PromptSubmission {
            kind,
            text,
            options,
            context,
            steer,
            edited,
        } = submission;
        let uuid = response["result"]["turn"]["id"].as_str();
        let completed = uuid
            .and_then(|id| self.turn_status_by_id.get(id))
            .is_some_and(|status| status.is_terminal());
        let mapped = *kind == BackendKind::Claude
            && uuid.is_some_and(|id| {
                self.has_item_mapping(&self.backend_entry_key(&format!("claude-prompt:{id}")))
            });
        if mapped {
            self.bind_claude_edit(text, options.origin, context.operation.as_ref(), response);
            self.receive_claude_edit_acceptance(uuid.expect("mapped UUID"), &mut update.changes);
        } else {
            if *steer {
                if kind.echoes_local_user_prompt() {
                    self.echo_prompt(
                        text,
                        Role::User,
                        options.record_history,
                        update,
                        context.operation.as_ref(),
                    );
                }
                if !completed {
                    self.set_status("sent steer");
                }
            } else {
                self.bind_claude_edit(text, options.origin, context.operation.as_ref(), response);
                self.mark_prompt_sent(submission, update, !completed);
            }
            if *kind == BackendKind::Claude {
                update.merge(self.receive_claude_submission(response));
            }
        }
        record_prompt_acceptance(update, submission);
        if *edited {
            if *kind == BackendKind::Codex {
                update.changes.items.reset_navigation |= self.record_prompt_history(text, None);
            }
            update.submitted = Some(text.clone());
        }
    }

    pub(super) fn prompt_failed(
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
        if self.reject_claude_edit_send(text, context.operation.as_ref()) {
            self.fail_claude_edit(&mut update.changes, error);
        } else if options.origin == PromptOrigin::Human {
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

    fn mark_prompt_sent(
        &mut self,
        submission: &PromptSubmission,
        update: &mut TurnUpdate,
        can_start: bool,
    ) {
        let kind = submission.kind;
        let text = &submission.text;
        let options = submission.options;
        let operation = submission.context.operation.as_ref();
        if kind.echoes_local_user_prompt() {
            let role = if kind == BackendKind::Claude && options.origin == PromptOrigin::Automatic {
                Role::System
            } else {
                Role::User
            };
            self.echo_prompt(text, role, options.record_history, update, operation);
            if can_start && self.active_turn_id().is_none() {
                self.mark_turn_started();
                self.set_active_turn_id(Some(
                    if kind == BackendKind::Grok {
                        crate::grok_backend::GROK_PENDING_TURN_ID
                    } else {
                        "claude-turn-pending"
                    }
                    .into(),
                ));
            }
        }
        if kind != BackendKind::Codex
            || !self.codex_scope().enabled()
            || self.codex_settings().legacy_overrides()
        {
            self.mark_runtime_settings_applied();
        }
        if can_start {
            self.set_status("sent turn");
        }
    }

    pub(super) fn echo_prompt(
        &mut self,
        text: &str,
        role: Role,
        record: bool,
        update: &mut TurnUpdate,
        operation: Option<&carlos_proto::submissions::ClientOperation>,
    ) {
        let index = self.append_message(role, text);
        self.bind_entry_operation(index, operation);
        if record {
            update.changes.items.reset_navigation |=
                self.record_prompt_history(text, Some(self.messages()[index].id));
        }
    }
}

fn record_prompt_acceptance(update: &mut TurnUpdate, submission: &PromptSubmission) {
    if submission.options.origin == PromptOrigin::Human {
        update
            .changes
            .effects
            .push(ClaudeEffect::Submission(SubmissionOutcome::PromptAccepted(
                submission.context.operation.clone(),
            )));
    }
}
