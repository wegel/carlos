//! Consume local editor changes returned by the Claude session owner.

use super::claude_question_state::NativeDraft;
use super::state::AppState;
use crate::session::actions::ServerRequestAction;
use crate::session::claude::callbacks::ClaudeCallback;
use crate::session::claude::receive_update::{ClaudeEffect, ClaudeUpdate};

impl AppState {
    /// Apply the returned controls once, then route a completed history fragment immediately.
    pub(super) fn apply_claude_update(
        &mut self,
        update: ClaudeUpdate,
    ) -> Option<ServerRequestAction> {
        for effect in update.effects {
            self.apply_claude_effect(effect);
        }
        self.apply_item_changes(update.items);
        if let Some(frame) = update.history_frame {
            let _ = super::notifications::handle_server_message_line(self, &frame.to_string());
        }
        update.action
    }

    /// Create exactly the local editor described by an accepted callback.
    pub(super) fn apply_claude_callback(&mut self, received: ClaudeCallback) {
        match received {
            ClaudeCallback::Ignored => {}
            ClaudeCallback::Unsupported(update) => self.apply_session_update(update),
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
            ClaudeEffect::Callback(callback) => self.apply_claude_callback(callback),
            ClaudeEffect::Cancelled {
                id,
                clear_confirmation,
            } => {
                if let Some(id) = id {
                    self.view.claude_answers.drafts.remove(&id);
                }
                if clear_confirmation {
                    self.view.confirm_allow = None;
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
            ClaudeEffect::EditFailed(failed) => self.view.claude_edit.failed = failed,
            ClaudeEffect::FollowBottom => self.view.viewport.auto_follow_bottom = true,
            ClaudeEffect::SetInput(text) => self.set_input_text(&text),
            ClaudeEffect::ClearInput => self.clear_input(),
            ClaudeEffect::ClearRewind => self.clear_rewind_mode_state(),
            ClaudeEffect::RetainDraft(text) => self.view.claude_edit.unsent_drafts.push_back(text),
            ClaudeEffect::LoadRetained => {
                if let Some(text) = self.view.claude_edit.unsent_drafts.pop_front() {
                    self.set_input_text(&text);
                }
            }
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
