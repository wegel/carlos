//! Change submitted prompt facts through the session owner.

use super::entry_id::EntryId;
use super::input_history::PromptHistory;
use super::state::SessionState;

impl SessionState {
    /// Borrow prompt records without exposing mutable session facts.
    pub(crate) fn input_history(&self) -> &PromptHistory {
        &self.input_history
    }

    /// Track native Codex user positions, including items without visible text.
    pub(crate) fn enable_native_history(&mut self) {
        self.input_history.enable_native();
    }

    /// Reset prompt records while keeping the backend's native-history mode.
    pub(crate) fn reset_prompt_history(&mut self) {
        let native = self.input_history.native_enabled();
        self.input_history = PromptHistory::default();
        if native {
            self.input_history.enable_native();
        }
    }

    /// Record prompt text and report whether the client must reset its cursor.
    pub(crate) fn record_prompt_history(&mut self, text: &str, entry: Option<EntryId>) -> bool {
        self.input_history.record_history(text, entry)
    }

    /// Remove only the matching unbound tail prompt and report whether it was removed.
    pub(crate) fn discard_pending_prompt(&mut self, text: &str) -> bool {
        self.input_history.discard_pending_history(text)
    }

    /// Attach a prompt entry to the backend turn used for rewind.
    pub(crate) fn record_prompt_turn(&mut self, entry: EntryId, turn: &str) {
        self.input_history.record_turn_id(entry, turn);
    }

    /// Deduplicate native user items before classifying a visible prompt as steering.
    pub(crate) fn record_native_user(
        &mut self,
        item: &str,
        turn: &str,
        entry: Option<EntryId>,
    ) -> Option<usize> {
        self.input_history.record_native(item, turn, entry)
    }

    #[cfg(test)]
    pub(crate) fn retain_prompt_entries(&mut self) {
        self.input_history.retain_entries(&self.messages);
    }
}
