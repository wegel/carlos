//! Adopt a checked fork before preparing the edited prompt's separate backend call.

use carlos_proto::submissions::SubmissionOutcome;
use serde_json::Value;

use super::claude::receive_update::ClaudeEffect;
use super::command_work::{PreparedCommand, PreparedStatus, PromptSubmission};
use super::models::{EntryDraft, Role};
use super::prompt_commands::PromptOptions;
use super::state::SessionState;
use crate::backend::BackendKind;

impl SessionState {
    pub(super) fn settle_rewind(
        &mut self,
        submission: PromptSubmission,
        prefix: Option<Vec<EntryDraft>>,
        result: Result<Value, String>,
    ) -> PreparedCommand {
        let mut prepared = PreparedCommand::default();
        let response = match result {
            Ok(response) => response,
            Err(error) => {
                if submission.kind == BackendKind::Claude {
                    self.fail_claude_edit(&mut prepared.update.changes, &error);
                }
                self.set_status(format!("fork failed: {error}"));
                self.append_message(Role::System, format!("Could not edit message: {error}"));
                prepared.status = PreparedStatus::Failed(error);
                return prepared;
            }
        };
        if submission.kind == BackendKind::Claude {
            prepared.status = PreparedStatus::Awaiting;
            prepared
                .update
                .changes
                .effects
                .push(ClaudeEffect::Submission(SubmissionOutcome::EditStarted(
                    submission.context.operation,
                )));
            self.retire_claude_queue(&mut prepared.update.changes);
            return prepared;
        }
        let response = response.to_string();
        let adopted = if let Some(prefix) = prefix {
            self.adopt_grok_fork(&response, prefix)
        } else {
            self.adopt_codex_fork(&response)
        };
        match adopted {
            Ok(changes) => prepared.update.merge(changes),
            Err(error) => {
                self.set_status(format!("fork failed: {error}"));
                prepared.status = PreparedStatus::Failed(error.to_string());
                return prepared;
            }
        }
        self.continue_edited_prompt(submission, prepared.update.changes)
    }

    fn continue_edited_prompt(
        &mut self,
        submission: PromptSubmission,
        changes: super::claude::receive_update::ClaudeUpdate,
    ) -> PreparedCommand {
        self.ralph_mark_user_turn_submitted();
        let next = self.prepare_prompt(
            submission.kind,
            submission.text,
            PromptOptions {
                force_start: true,
                ..submission.options
            },
            &submission.context,
            true,
        );
        let mut next = next.expect("the completed fork released its command slot");
        next.update.changes.effects.splice(0..0, changes.effects);
        next.update.changes.items.reset_navigation |= changes.items.reset_navigation;
        next.update.changes.items.history_replaced |= changes.items.history_replaced;
        next.update.rewound = true;
        next.update.clear_selection = true;
        next
    }
}
