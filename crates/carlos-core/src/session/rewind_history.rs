//! Adopt complete fork histories only after checking the backend response.

use anyhow::Context;

use crate::protocol_params::{extract_result_object, parse_thread_id_from_start_or_resume};

use super::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use super::history_identity::HistoryMode;
use super::models::{EntryDraft, Role};
use super::state::SessionState;

impl SessionState {
    /// Adopt the copied Grok prefix with fresh ids only after its fork succeeds.
    pub fn adopt_grok_fork(
        &mut self,
        response: &str,
        prefix: Vec<EntryDraft>,
    ) -> anyhow::Result<ClaudeUpdate> {
        let id =
            parse_thread_id_from_start_or_resume(response).context("missing rewound thread id")?;
        let mut update = self.reset_fork(id);
        for row in prefix {
            let record = row.role == Role::User && !row.text.is_empty();
            let text = row.text.clone();
            let id = self.transcript_append(row);
            if record {
                update.items.reset_navigation |= self.record_prompt_history(&text, Some(id));
            }
            update.items.push(super::transcript_updates::SessionUpdate {
                dirty_from: Some(id),
                transcript_reset: false,
            });
        }
        Ok(update)
    }

    /// Parse the full Codex response before replacing any old transcript fact.
    pub fn adopt_codex_fork(&mut self, response: &str) -> anyhow::Result<ClaudeUpdate> {
        let id =
            parse_thread_id_from_start_or_resume(response).context("missing forked thread id")?;
        let parsed = extract_result_object(response)?;
        let thread = &parsed["result"]["thread"];
        let previous = self.replace_codex_item_turn(None);
        let mut draft = super::item_history_draft::build(self, thread);
        draft.set_conversation(&id);
        self.replace_codex_item_turn(previous);
        let mut update = self.reset_fork(id);
        let changes = self.replace_native_history(draft, HistoryMode::NewConversation);
        for change in changes.transcript {
            update.items.push(change);
        }
        update.items.reset_navigation |= changes.reset_navigation;
        self.observe_thread_name(thread);
        Ok(update)
    }

    fn reset_fork(&mut self, id: String) -> ClaudeUpdate {
        let mut update = ClaudeUpdate::default();
        update
            .effects
            .push(ClaudeEffect::Thread(self.adopt_native_thread(id)));
        self.reset_for_forked_thread();
        self.reset_prompt_history();
        update.effects.push(ClaudeEffect::HistoryReset);
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
        update
    }
}
