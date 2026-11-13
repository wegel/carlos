//! Adopt native Claude switches and clears before returning local control resets.

use serde_json::Value;

use super::receive_update::{ClaudeDraftContext, ClaudeEffect, ClaudeUpdate};
use crate::session::approval_state::ApprovalQueue;
use crate::session::models::Role;
use crate::session::state::SessionState;

impl SessionState {
    /// Start a new helper generation while preserving the client's retained unsent drafts.
    pub(crate) fn switch_claude_session(
        &mut self,
        generation: u64,
        id: &str,
        update: &mut ClaudeUpdate,
    ) {
        self.retire_claude_queue(update);
        let edit = self.take_claude_edit_prompt();
        self.switch_claude_runtime_choices();
        self.reset_claude_thread(id.to_string(), false, update);
        self.reset_claude_for_switch(generation);
        update.effects.push(ClaudeEffect::SwitchControls);
        update.effects.push(ClaudeEffect::Cards(
            self.clear_approval_queue(ApprovalQueue::Claude),
        ));
        self.resume_claude_edit(edit);
        self.set_available_models(Vec::new());
        self.set_status("Claude starts the edited branch");
    }

    /// Accept one native clear key while preserving active callbacks and process tasks.
    pub(crate) fn receive_claude_reset(&mut self, message: &Value, update: &mut ClaudeUpdate) {
        if !self.begin_claude_reset(message) {
            return;
        }
        self.reset_claude_thread(self.thread_id().clone(), true, update);
        self.set_thread_name(None);
        self.clear_claude_history();
        update.effects.push(ClaudeEffect::ResetControls);
        self.finish_claude_reset();
    }

    /// Adopt a clear's accepted native identity before recording its observed model.
    pub(crate) fn receive_claude_initialized(
        &mut self,
        message: &Value,
        update: &mut ClaudeUpdate,
    ) {
        if let Some(id) = self.claude_reset_identity(message) {
            let changed = self.adopt_native_thread(id.to_string());
            update.effects.push(ClaudeEffect::Thread(changed));
            self.finish_claude_reset_init();
        }
        if let Some(model) = message["model"].as_str() {
            self.observe_claude_model(model);
        }
    }

    /// Queue the saved edit only after accepted capabilities make the helper ready.
    pub(crate) fn receive_claude_capabilities(
        &mut self,
        capabilities: &Value,
        draft: &ClaudeDraftContext<'_>,
        update: &mut ClaudeUpdate,
    ) {
        if !self.apply_claude_capabilities(capabilities) {
            return;
        }
        if let Some(text) = self.take_claude_edit_prompt() {
            self.retire_claude_queue(update);
            self.queue_claude_edit(&text);
            update.effects.push(ClaudeEffect::EditFailed(false));
            update.effects.push(ClaudeEffect::ClearRewind);
            if update.composer_matches(draft, &text) {
                update.effects.push(ClaudeEffect::ClearInput);
            }
            self.queue_turn_input_with_history(text, true);
        }
    }

    /// Bind a submitted UUID and preserve the local failed-badge precedence.
    pub(crate) fn receive_claude_submission(
        &mut self,
        response: &Value,
        failed: bool,
    ) -> ClaudeUpdate {
        let mut update = ClaudeUpdate::default();
        let Some(id) = response["result"]["turn"]["id"].as_str() else {
            return update;
        };
        let key = self.backend_entry_key(&format!("claude-prompt:{id}"));
        update
            .items
            .push(self.record_submitted_claude_prompt(response, &key));
        self.receive_claude_edit_acceptance(id, failed, &mut update);
        update
    }

    /// Dismiss a prior local failure before accepting the matching submitted UUID.
    pub(crate) fn receive_claude_edit_acceptance(
        &mut self,
        id: &str,
        failed: bool,
        update: &mut ClaudeUpdate,
    ) {
        if failed {
            update.effects.push(ClaudeEffect::EditFailed(false));
        } else if self.accept_claude_edit(id) {
            update.items.track(
                self.append_message(Role::System, "Claude started the edited branch".to_string()),
            );
        }
    }

    fn reset_claude_thread(&mut self, id: String, preserve: bool, update: &mut ClaudeUpdate) {
        let changed = self.adopt_native_thread(id);
        update.effects.push(ClaudeEffect::Thread(changed));
        self.reset_for_forked_thread();
        self.reset_prompt_history();
        update.effects.push(ClaudeEffect::HistoryReset);
        if preserve {
            return;
        }
        if self.codex_scope().enabled() {
            for id in self.codex_callback_ids() {
                update
                    .effects
                    .push(ClaudeEffect::CodexRetired(self.retire_codex_callback(&id)));
            }
            update.effects.push(ClaudeEffect::CodexUnfocus);
        } else {
            update
                .effects
                .push(ClaudeEffect::Cleared(self.clear_pending_session_approval()));
        }
    }
}
