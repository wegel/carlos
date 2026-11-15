//! Keep prompt navigation, rewind focus, and unsent drafts in the terminal view.

use carlos_core::session::entry_id::EntryId;
use carlos_core::session::input_history::PromptHistory;

/// Track one client's cursor without changing submitted prompt facts.
#[derive(Default)]
pub(super) struct InputHistoryView {
    input_history_index: Option<usize>,
    selected_entry_id: Option<EntryId>,
    selected_prompt_id: Option<EntryId>,
    input_history_draft: Option<String>,
    rewind_mode: bool,
    rewind_restore_draft: Option<String>,
}

impl InputHistoryView {
    // --- Mode Control ---
    /// Report whether this client is editing a submitted prompt.
    pub(super) fn rewind_mode(&self) -> bool {
        self.rewind_mode
    }

    /// Drop the cursor and its saved navigation draft.
    pub(super) fn reset_navigation(&mut self) {
        self.input_history_index = None;
        self.selected_entry_id = None;
        self.selected_prompt_id = None;
        self.input_history_draft = None;
    }

    /// Save the composer once before selecting an earlier prompt.
    pub(super) fn enter_rewind_mode(&mut self, current_input_text: String) -> bool {
        if self.rewind_mode {
            return false;
        }
        self.rewind_mode = true;
        self.rewind_restore_draft = Some(current_input_text);
        self.reset_navigation();
        true
    }

    /// Leave rewind and recover its original composer draft.
    pub(super) fn exit_rewind_mode_restore(&mut self) -> Option<String> {
        if !self.rewind_mode {
            return None;
        }
        let draft = self.rewind_restore_draft.take().unwrap_or_default();
        self.rewind_mode = false;
        self.reset_navigation();
        Some(draft)
    }

    /// Leave rewind while keeping the existing navigation cursor.
    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.rewind_mode = false;
        self.rewind_restore_draft = None;
    }

    // --- History Navigation ---
    /// Select the previous prompt, saving the composer on the first move.
    pub(super) fn navigate_up(
        &mut self,
        history: &PromptHistory,
        current_input_text: String,
    ) -> Option<String> {
        if history.history_len() == 0 {
            return None;
        }

        let next_idx = match self.input_history_index {
            Some(0) => 0,
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.input_history_draft = Some(current_input_text);
                history.history_len().saturating_sub(1)
            }
        };

        self.input_history_index = Some(next_idx);
        self.selected_entry_id = history.entry_id(next_idx);
        self.selected_prompt_id = history.prompt_id(next_idx);
        Some(history.text(next_idx).to_string())
    }

    /// Select the next prompt or restore the saved composer at the end.
    pub(super) fn navigate_down(&mut self, history: &PromptHistory) -> Option<String> {
        let idx = self.input_history_index?;

        if idx + 1 < history.history_len() {
            let next_idx = idx + 1;
            self.input_history_index = Some(next_idx);
            self.selected_entry_id = history.entry_id(next_idx);
            self.selected_prompt_id = history.prompt_id(next_idx);
            return Some(history.text(next_idx).to_string());
        }

        let draft = self.input_history_draft.take().unwrap_or_default();
        self.input_history_index = None;
        self.selected_entry_id = None;
        self.selected_prompt_id = None;
        Some(draft)
    }

    // --- Rewind Selection ---
    /// Read the stable prompt selected by this client, including an unbound draft.
    pub(super) fn rewind_prompt_id(&self) -> Option<EntryId> {
        self.selected_prompt_id
    }

    /// Read the entry selected by this client.
    pub(super) fn rewind_selected_entry_id(&self) -> Option<EntryId> {
        self.selected_entry_id
    }
}
