//! Adopt complete histories and retire forked transcript facts as one owner.

use super::history_identity::{HistoryEntry, HistoryMode};
use super::state::SessionState;
use super::transcript_updates::SessionUpdate;

impl SessionState {
    /// Adopt checked drafts before the app restores its still-mixed history helpers.
    pub(crate) fn replace_history(
        &mut self,
        entries: Vec<HistoryEntry>,
        mode: HistoryMode,
    ) -> SessionUpdate {
        self.summaries.clear();
        self.messages.replace_history(entries, mode);
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.command_render_overrides.clear();
        SessionUpdate {
            transcript_reset: true,
            ..SessionUpdate::default()
        }
    }

    /// Retire the old conversation after the app accepts a fork or clear.
    pub(crate) fn reset_for_forked_thread(&mut self) -> SessionUpdate {
        self.entry_conversation = self.thread_id.clone();
        self.active_turn_id = None;
        self.messages.reset();
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.command_render_overrides.clear();
        self.turn_boundary = None;
        self.context_usage = None;
        self.summaries.clear();
        SessionUpdate {
            transcript_reset: true,
            ..SessionUpdate::default()
        }
    }

    /// Remove a suffix without dirtying an earlier retained entry.
    #[cfg(test)]
    pub(crate) fn truncate_at(&mut self, index: usize) -> usize {
        if let Some(id) = self.messages.get(index).map(|message| message.id) {
            self.messages.truncate_before(id);
        }
        self.command_render_overrides.clear();
        self.messages.len()
    }
}
