//! Preserve submitted Claude edits and queued human drafts across helper failures.

use super::receive_update::{ClaudeDraftContext, ClaudeEffect, ClaudeUpdate};
use crate::session::approval_state::ApprovalQueue;
use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::text::safe_text;

impl SessionState {
    /// Fail helper work once, returning unsent text without replacing a newer draft.
    pub fn fail_claude(&mut self, draft: &ClaudeDraftContext<'_>, update: &mut ClaudeUpdate) {
        self.recover_sending_claude_edit();
        self.restore_claude_edit(draft, update);
        if self.close_claude() {
            update.effects.push(ClaudeEffect::EditFailed(true));
        }
        update.effects.push(ClaudeEffect::Cards(
            self.clear_approval_queue(ApprovalQueue::Claude),
        ));
        update
            .effects
            .push(ClaudeEffect::Cleared(self.clear_pending_session_approval()));
        self.fail_claude_session_work();
        self.retain_claude_drafts(update);
        if update.composer_is_empty(draft) {
            update.effects.push(ClaudeEffect::LoadRetained);
        }
    }

    /// Return the saved edit once, retaining a newer client draft and its recovery notice.
    pub(crate) fn restore_claude_edit(
        &mut self,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        let Some(text) = self.take_claude_edit_prompt() else {
            return;
        };
        if update.composer_is_empty(draft) {
            update.effects.push(ClaudeEffect::SetInput(text));
        } else if !update.composer_matches(draft, &text) {
            update.items.track(self.append_message(
                Role::System,
                format!("Unsent edited draft (F5 loads it when the composer is empty):\n{text}"),
            ));
            update.effects.push(ClaudeEffect::RetainDraft(text));
        }
    }

    /// Stop automatic work and transfer queued human text to explicit client recovery.
    pub fn retire_claude_queue(&mut self, update: &mut ClaudeUpdate) {
        self.ralph_stop_after_failure();
        self.retain_claude_drafts(update);
    }

    /// Transfer queued human drafts in their original order with the existing notices.
    pub(crate) fn retain_claude_drafts(&mut self, update: &mut ClaudeUpdate) {
        let (drafts, reset_navigation) = self.retain_claude_queued_drafts();
        update.items.reset_navigation |= reset_navigation;
        for text in drafts {
            update.items.track(self.append_message(
                Role::System,
                format!("Unsent queued draft (F5 loads it when the composer is empty):\n{text}"),
            ));
            update.effects.push(ClaudeEffect::RetainDraft(text));
        }
    }

    /// End a failed fork without reopening a helper that has already closed.
    pub fn fail_claude_edit(
        &mut self,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
        message: &str,
    ) {
        self.restore_claude_edit(draft, update);
        self.ready_claude_after_edit_failure();
        self.unavailable_claude_edit(update, message);
    }

    /// Explain an unavailable edit while preserving the helper's current phase.
    pub fn unavailable_claude_edit(&mut self, update: &mut ClaudeUpdate, message: &str) {
        self.end_claude_edit();
        update.effects.push(ClaudeEffect::EditFailed(true));
        let message = safe_text(message);
        update.items.track(self.append_message(
            Role::System,
            format!("Could not edit Claude message: {message}. Draft kept."),
        ));
        update.effects.push(ClaudeEffect::FollowBottom);
    }
}
