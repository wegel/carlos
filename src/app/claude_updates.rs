//! Consume local editor changes returned by the Claude session owner.

use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::claude::callbacks::ClaudeCallback;
use carlos_core::session::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use carlos_proto::submissions::{RecoveryKind, SubmissionOutcome, SubmittedText};

use super::claude_question_state::NativeDraft;
use super::state::AppState;

impl AppState {
    /// Apply the returned controls once, then route a completed history fragment immediately.
    pub(super) fn apply_claude_update(
        &mut self,
        update: ClaudeUpdate,
    ) -> Option<ServerRequestAction> {
        let closed = update
            .effects
            .iter()
            .any(|effect| matches!(effect, ClaudeEffect::SessionClosed));
        for effect in update.effects {
            self.apply_claude_effect(effect);
        }
        if closed && self.input_is_empty() {
            if let Some(text) = self.view.claude_edit.unsent_drafts.pop_front() {
                self.set_input_text(&text);
            }
        }
        self.apply_item_changes(update.items);
        update.action
    }

    /// Create exactly the local editor described by an accepted callback.
    pub(super) fn apply_claude_callback(&mut self, received: ClaudeCallback) {
        match received {
            ClaudeCallback::Ignored => {}
            ClaudeCallback::Unsupported => self.consume_core_events(),
            ClaudeCallback::Accepted {
                id,
                card,
                description,
                retired,
            } => {
                let draft = NativeDraft::from_description(description);
                let has_fields = !draft.fields.is_empty();
                self.view.claude_answers.drafts.insert(id, draft);
                self.drop_retired_approval_hints(retired);
                self.view.approval_hints.register_callback(card, has_fields);
            }
        }
    }

    fn apply_claude_effect(&mut self, effect: ClaudeEffect) {
        match effect {
            ClaudeEffect::TaskStop(_) => {}
            ClaudeEffect::Callback(callback) => self.apply_claude_callback(callback),
            ClaudeEffect::Cancelled { id, card } => {
                if card.is_some() && self.view.confirm_allow == card {
                    self.view.confirm_allow = None;
                }
                if let Some(id) = id {
                    self.view.claude_answers.drafts.remove(&id);
                }
            }
            ClaudeEffect::Browser(id) => {
                if let Some(draft) = id
                    .as_deref()
                    .and_then(|id| self.view.claude_answers.drafts.get_mut(id))
                {
                    draft.notice = Some("The server reported browser completion. Your consent is still required; decline or cancel remains available.".into());
                }
            }
            ClaudeEffect::Thread(changed) => self.apply_thread_change(changed),
            ClaudeEffect::HistoryReset => self.reset_history_view(),
            ClaudeEffect::Cleared(cleared) => self.apply_cleared_approval(cleared),
            ClaudeEffect::Cards(cards) => self.drop_retired_approval_hints(cards),
            ClaudeEffect::CodexRetired(retired) => {
                super::codex_callbacks::apply_retired(self, retired)
            }
            ClaudeEffect::CodexUnfocus => self.view.codex_callbacks.focused = false,
            ClaudeEffect::SwitchControls => self.switch_claude_controls(),
            ClaudeEffect::ResetControls => {
                self.view.claude_unknown_selection = None;
                self.view.claude_edit = Default::default();
            }
            ClaudeEffect::Submission(outcome) => self.apply_submission_outcome(outcome),
            ClaudeEffect::EditAdopted(_) => {}
            ClaudeEffect::SessionClosed => self.view.claude_edit.failed = true,
        }
    }

    fn apply_submission_outcome(&mut self, outcome: SubmissionOutcome) {
        match outcome {
            SubmissionOutcome::EditStarted(operation) => {
                if self.view.owns_operation(operation.as_ref()) {
                    self.view.claude_edit.failed = false;
                    self.view.viewport.auto_follow_bottom = true;
                }
            }
            SubmissionOutcome::EditRejected(operation) => {
                if self.view.owns_operation(operation.as_ref()) {
                    self.view.claude_edit.failed = true;
                    self.view.viewport.auto_follow_bottom = true;
                }
            }
            SubmissionOutcome::PromptAccepted(operation)
            | SubmissionOutcome::EditAccepted(operation) => {
                if self.view.owns_operation(operation.as_ref()) {
                    self.view.claude_edit.failed = false;
                }
            }
            SubmissionOutcome::EditQueued(draft) => self.apply_queued_edit(draft),
            SubmissionOutcome::Recovered { draft, kind } => {
                self.recover_submitted_text(draft, kind)
            }
        }
    }

    fn apply_queued_edit(&mut self, draft: SubmittedText) {
        if !self.view.owns_operation(draft.operation.as_ref()) {
            return;
        }
        self.view.claude_edit.failed = false;
        self.clear_rewind_mode_state();
        if self.input_text() == draft.text {
            self.clear_input();
        }
    }

    fn recover_submitted_text(&mut self, draft: SubmittedText, kind: RecoveryKind) {
        if !self.view.owns_operation(draft.operation.as_ref()) {
            return;
        }
        if kind != RecoveryKind::Queued && self.input_is_empty() {
            self.set_input_text(&draft.text);
        } else if kind == RecoveryKind::Queued || self.input_text() != draft.text {
            self.view.claude_edit.unsent_drafts.push_back(draft.text);
            self.set_hint("Unsent draft retained; F5 loads it when the composer is empty");
        }
    }

    fn switch_claude_controls(&mut self) {
        self.view.claude_answers = Default::default();
        self.view.claude_tasks = Default::default();
        self.view.claude_queue = false;
        self.view.claude_unknown_selection = None;
        self.view.claude_quota_next_draw = None;
        self.view.confirm_allow = None;
        let unsent = std::mem::take(&mut self.view.claude_edit.unsent_drafts);
        self.view.claude_edit = Default::default();
        self.view.claude_edit.unsent_drafts = unsent;
    }
}
