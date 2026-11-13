//! Adopt complete histories and retire forked transcript facts as one owner.

use super::history_identity::{HistoryEntry, HistoryMode};
use super::state::SessionState;

impl SessionState {
    /// Adopt checked drafts before the app restores its still-mixed history helpers.
    pub fn replace_history(&mut self, entries: Vec<HistoryEntry>, mode: HistoryMode) {
        self.summaries.clear();
        self.messages.replace_history(entries, mode);
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.command_render_overrides.clear();
    }

    /// Retire the old conversation after the app accepts a fork or clear.
    pub fn reset_for_forked_thread(&mut self) {
        self.retire_backend_command();
        self.entry_conversation = self.thread_id.clone();
        self.active_turn_id = None;
        self.messages.reset();
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.command_render_overrides.clear();
        self.turn_boundary = None;
        self.context_usage = None;
        self.summaries.clear();
    }

    /// Remove a suffix without dirtying an earlier retained entry.
    #[cfg(any(test, feature = "test-support"))]
    pub fn truncate_at(&mut self, index: usize) -> usize {
        if let Some(id) = self.messages.get(index).map(|message| message.id) {
            self.messages.truncate_before(id);
        }
        self.command_render_overrides.clear();
        self.messages.len()
    }
}
