//! Input history navigation and rewind-mode state.

use std::collections::{HashMap, HashSet};

use super::input_history_native::{NativeUserHistory, NativeUserPosition};
use crate::backend::RewindUserMessage;
use crate::session::entry_id::EntryId;
#[cfg(test)]
use crate::session::transcript::Transcript;

// --- History State ---

/// Tracks submitted prompts and supports up/down history navigation.
pub(super) struct InputHistoryState {
    input_history: Vec<String>,
    input_history_entry_ids: Vec<Option<EntryId>>,
    input_history_index: Option<usize>,
    input_history_draft: Option<String>,
    rewind_mode: bool,
    rewind_restore_draft: Option<String>,
    turn_ids_by_message: HashMap<EntryId, String>,
    native: Option<NativeUserHistory>,
    recorded: HashSet<EntryId>,
}

impl InputHistoryState {
    pub(super) fn new() -> Self {
        Self {
            input_history: Vec::new(),
            input_history_entry_ids: Vec::new(),
            input_history_index: None,
            input_history_draft: None,
            rewind_mode: false,
            rewind_restore_draft: None,
            turn_ids_by_message: HashMap::new(),
            native: None,
            recorded: HashSet::new(),
        }
    }

    /// Enable Codex user-item metadata without changing Claude or Grok history.
    pub(super) fn enable_native(&mut self) {
        self.native.get_or_insert_with(NativeUserHistory::default);
    }

    /// Report whether replacement history must also track Codex identities.
    pub(super) fn native_enabled(&self) -> bool {
        self.native.is_some()
    }

    /// Record every native user item before choosing its visible row label.
    pub(super) fn record_native(
        &mut self,
        item_id: &str,
        turn_id: &str,
        message: Option<EntryId>,
    ) -> Option<usize> {
        self.native
            .as_mut()
            .map(|native| native.record(item_id, turn_id, message))
    }

    /// Resolve the selected row's exact native position, independently of its text.
    pub(super) fn selected_native(&self) -> Option<&NativeUserPosition> {
        self.native
            .as_ref()?
            .message(self.rewind_selected_entry_id()?)
    }

    /// Retain the original selected text when the composer contains an edited draft.
    pub(super) fn selected_native_request(&self) -> Option<RewindUserMessage> {
        let selected = self.selected_native()?;
        Some(RewindUserMessage {
            item_id: selected.item_id.clone(),
            position: selected.position,
            text: self.input_history.get(self.input_history_index?)?.clone(),
        })
    }

    // --- Mode Control ---
    pub(super) fn rewind_mode(&self) -> bool {
        self.rewind_mode
    }

    pub(super) fn reset_navigation(&mut self) {
        self.input_history_index = None;
        self.input_history_draft = None;
    }

    pub(super) fn enter_rewind_mode(&mut self, current_input_text: String) -> bool {
        if self.rewind_mode {
            return false;
        }
        self.rewind_mode = true;
        self.rewind_restore_draft = Some(current_input_text);
        self.reset_navigation();
        true
    }

    pub(super) fn exit_rewind_mode_restore(&mut self) -> Option<String> {
        if !self.rewind_mode {
            return None;
        }
        let draft = self.rewind_restore_draft.take().unwrap_or_default();
        self.rewind_mode = false;
        self.reset_navigation();
        Some(draft)
    }

    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.rewind_mode = false;
        self.rewind_restore_draft = None;
    }

    // --- History Recording ---
    pub(super) fn push_history(&mut self, text: &str) {
        self.record_history(text, None);
    }

    pub(super) fn discard_pending_history(&mut self, text: &str) {
        if self
            .input_history
            .last()
            .is_some_and(|pending| pending == text)
            && self.input_history_entry_ids.last() == Some(&None)
        {
            self.input_history.pop();
            self.input_history_entry_ids.pop();
            self.reset_navigation();
        }
    }

    pub(super) fn record_history(&mut self, text: &str, message_idx: Option<EntryId>) {
        if text.is_empty() {
            self.reset_navigation();
            return;
        }

        if let Some(msg_idx) = message_idx {
            if !self.recorded.insert(msg_idx) {
                return;
            }
            if let (Some(last_text), Some(last_idx)) = (
                self.input_history.last(),
                self.input_history_entry_ids.last_mut(),
            ) {
                if *last_text == text && last_idx.is_none() {
                    *last_idx = Some(msg_idx);
                    self.reset_navigation();
                    return;
                }
            }
        }

        self.input_history.push(text.to_string());
        self.input_history_entry_ids.push(message_idx);
        self.reset_navigation();
    }

    // --- History Navigation ---
    pub(super) fn navigate_up(&mut self, current_input_text: String) -> Option<String> {
        if self.input_history.is_empty() {
            return None;
        }

        let next_idx = match self.input_history_index {
            Some(0) => 0,
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.input_history_draft = Some(current_input_text);
                self.input_history.len().saturating_sub(1)
            }
        };

        self.input_history_index = Some(next_idx);
        Some(self.input_history[next_idx].clone())
    }

    pub(super) fn navigate_down(&mut self) -> Option<String> {
        let idx = self.input_history_index?;

        if idx + 1 < self.input_history.len() {
            let next_idx = idx + 1;
            self.input_history_index = Some(next_idx);
            return Some(self.input_history[next_idx].clone());
        }

        let draft = self.input_history_draft.take().unwrap_or_default();
        self.input_history_index = None;
        Some(draft)
    }

    // --- Rewind Selection ---
    pub(super) fn rewind_selected_entry_id(&self) -> Option<EntryId> {
        let idx = self.input_history_index?;
        self.input_history_entry_ids.get(idx).and_then(|v| *v)
    }

    pub(super) fn rewind_selected_history_index(&self) -> Option<usize> {
        self.input_history_index
    }

    pub(super) fn history_len(&self) -> usize {
        self.input_history.len()
    }

    pub(super) fn record_turn_id(&mut self, message_idx: EntryId, turn_id: &str) {
        self.turn_ids_by_message
            .insert(message_idx, turn_id.to_string());
    }

    pub(super) fn selected_turn_id(&self) -> Option<&str> {
        if let Some(selected) = self.selected_native() {
            return Some(&selected.turn_id);
        }
        let message_idx = self.rewind_selected_entry_id()?;
        self.turn_ids_by_message
            .get(&message_idx)
            .map(String::as_str)
    }

    #[cfg(test)]
    pub(super) fn retain_entries(&mut self, transcript: &Transcript) {
        for id in &mut self.input_history_entry_ids {
            if id.is_some_and(|id| transcript.entry(id).is_none()) {
                *id = None;
            }
        }
        self.recorded.retain(|id| transcript.entry(*id).is_some());
        self.turn_ids_by_message
            .retain(|id, _| transcript.entry(*id).is_some());
        if let Some(native) = &mut self.native {
            native.retain_entries(transcript);
        }
    }

    // --- Test Helpers ---
    #[cfg(test)]
    pub(super) fn message_ids(&self) -> &[Option<EntryId>] {
        &self.input_history_entry_ids
    }

    #[cfg(test)]
    pub(super) fn set_rewind_selection(&mut self, index: Option<usize>) {
        self.rewind_mode = true;
        self.input_history_index = index;
    }
}

#[cfg(test)]
impl InputHistoryState {
    pub(super) fn assert_invariants(&self, transcript: &Transcript) {
        assert_eq!(self.input_history.len(), self.input_history_entry_ids.len());
        for id in self
            .input_history_entry_ids
            .iter()
            .flatten()
            .chain(self.turn_ids_by_message.keys())
            .chain(self.recorded.iter())
        {
            assert!(transcript.entry(*id).is_some());
        }
        if let Some(native) = &self.native {
            native.assert_invariants(transcript);
        }
    }
}
