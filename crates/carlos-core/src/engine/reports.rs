//! Consume shared reports, private submission facts and device hints at the owner boundary.

use carlos_proto::submissions::{OperationState, SubmissionOutcome};

use super::owner::Owner;
use crate::session::backend_update::{BackendChange, BackendUpdate};
use crate::session::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use crate::session::codex::callback_update::{CallbackEffect, CallbackUpdate};
use crate::session::prompt_commands::TurnUpdate;

impl Owner {
    pub(super) fn report(&mut self, report: BackendUpdate) {
        match report.change {
            BackendChange::None | BackendChange::Items(_) => {}
            BackendChange::Installed(installed) => {
                debug_assert!(installed.editor.is_none());
            }
            BackendChange::Common(report) => {
                if let Some(action) = report.action {
                    self.action(action);
                }
            }
            BackendChange::Claude(report) => self.claude_report(report),
            BackendChange::Codex(report) => self.codex_report(report),
            BackendChange::Closed(_) => {
                // Native queue/settings mutation families have no remote producer before EP019b.
                self.close_native();
            }
        }
        if let Some(nested) = report.nested {
            self.report(*nested);
        }
    }

    pub(super) fn turn_report(&mut self, report: TurnUpdate) {
        self.claude_report(report.changes);
    }

    fn claude_report(&mut self, report: ClaudeUpdate) {
        debug_assert!(
            report.history_frame.is_none(),
            "history is assembled by finish_claude_update before routing"
        );
        for effect in report.effects {
            match effect {
                ClaudeEffect::Submission(outcome) => self.submission(outcome),
                ClaudeEffect::SessionClosed => {
                    self.close_native();
                }
                ClaudeEffect::Callback(_)
                | ClaudeEffect::Cancelled { .. }
                | ClaudeEffect::Browser(_)
                | ClaudeEffect::Thread(_)
                | ClaudeEffect::HistoryReset
                | ClaudeEffect::Cleared(_)
                | ClaudeEffect::Cards(_)
                | ClaudeEffect::CodexRetired(_) => { /* core already recorded shared facts */ }
                ClaudeEffect::CodexUnfocus
                | ClaudeEffect::SwitchControls
                | ClaudeEffect::ResetControls => { /* device hints */ }
            }
        }
        if let Some(action) = report.action {
            self.action(action);
        }
    }

    fn codex_report(&mut self, report: CallbackUpdate) {
        match report.effect {
            CallbackEffect::None
            | CallbackEffect::Accepted { .. }
            | CallbackEffect::Retired(_)
            | CallbackEffect::Items(_)
            | CallbackEffect::Queue(_)
            | CallbackEffect::Panel(_)
            | CallbackEffect::Controls { .. } => { /* shared facts or EP019b device hints */ }
            CallbackEffect::Goal(update) => self.finish_goal(update),
            CallbackEffect::ReplyFailed { error, .. } => self.session.set_status(error),
            CallbackEffect::Checked { update, .. } => {
                if let Some((card_id, result)) = update.reply {
                    self.action(crate::session::actions::ServerRequestAction::ReplyNative {
                        card_id,
                        result,
                    });
                }
            }
        }
        if let Some(action) = report.action {
            self.action(action);
        }
    }

    fn submission(&mut self, outcome: SubmissionOutcome) {
        match outcome {
            SubmissionOutcome::PromptAccepted(Some(operation))
            | SubmissionOutcome::EditAccepted(Some(operation)) => {
                self.results
                    .update(&operation, OperationState::Accepted { entry: None });
            }
            SubmissionOutcome::Recovered { draft, .. } => self.results.recover(
                draft,
                "native send was not confirmed; do not retry automatically",
                true,
            ),
            SubmissionOutcome::EditQueued(draft) => {
                if let Some(operation) = draft.operation {
                    self.results.update(&operation, OperationState::Queued);
                }
            }
            SubmissionOutcome::EditRejected(Some(operation)) => self.results.update(
                &operation,
                super::dispatch::rejected("native edit rejected"),
            ),
            SubmissionOutcome::PromptAccepted(None)
            | SubmissionOutcome::EditAccepted(None)
            | SubmissionOutcome::EditStarted(_)
            | SubmissionOutcome::EditRejected(None) => {}
        }
        self.notice();
    }
}
