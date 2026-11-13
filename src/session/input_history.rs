//! Keep submitted prompts and backend rewind positions independently of a client cursor.

use std::collections::{HashMap, HashSet};

use crate::backend::RewindUserMessage;

use super::entry_id::{EntryId, EntryIds};
use super::input_history_native::{NativeUserHistory, NativeUserPosition};
#[cfg(test)]
use super::transcript::Transcript;

/// Own prompt records, their entry links, and native backend positions.
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct PromptHistory {
    input_history: Vec<String>,
    prompt_ids: Vec<EntryId>,
    input_history_entry_ids: Vec<Option<EntryId>>,
    turn_ids_by_message: HashMap<EntryId, String>,
    native: Option<NativeUserHistory>,
    recorded: HashSet<EntryId>,
}

impl PromptHistory {
    pub(in crate::session) fn enable_native(&mut self) {
        self.native.get_or_insert_with(NativeUserHistory::default);
    }

    /// Report whether history must track native Codex identities.
    pub(crate) fn native_enabled(&self) -> bool {
        self.native.is_some()
    }

    pub(in crate::session) fn record_native(
        &mut self,
        item: &str,
        turn: &str,
        entry: Option<EntryId>,
    ) -> Option<usize> {
        self.native
            .as_mut()
            .map(|native| native.record(item, turn, entry))
    }

    /// Borrow the native position attached to a visible user entry.
    pub(crate) fn native_user(&self, entry: EntryId) -> Option<&NativeUserPosition> {
        self.native.as_ref()?.message(entry)
    }

    /// Keep the original prompt text when a client edits its composer before rewind.
    pub(crate) fn native_request(&self, index: usize, entry: EntryId) -> Option<RewindUserMessage> {
        let selected = self.native_user(entry)?;
        Some(RewindUserMessage {
            item_id: selected.item_id.clone(),
            position: selected.position,
            text: self.input_history.get(index)?.clone(),
        })
    }

    /// Borrow one prompt's original text.
    pub(crate) fn text(&self, index: usize) -> &str {
        &self.input_history[index]
    }

    /// Identify a prompt even before the backend supplies its transcript row.
    pub(crate) fn prompt_id(&self, index: usize) -> Option<EntryId> {
        self.prompt_ids.get(index).copied()
    }

    /// Resolve a selected prompt inside the owner rather than trusting a client count.
    pub(in crate::session) fn prompt_index(&self, id: EntryId) -> Option<usize> {
        self.prompt_ids
            .iter()
            .position(|candidate| *candidate == id)
            .or_else(|| {
                self.input_history_entry_ids
                    .iter()
                    .position(|entry| *entry == Some(id))
            })
    }

    /// Resolve a prompt record to its optional transcript entry.
    pub(crate) fn entry_id(&self, index: usize) -> Option<EntryId> {
        self.input_history_entry_ids.get(index).copied().flatten()
    }

    /// Count submitted and pending prompt records.
    pub(crate) fn history_len(&self) -> usize {
        self.input_history.len()
    }

    pub(in crate::session) fn record_turn_id(&mut self, entry: EntryId, turn: &str) {
        self.turn_ids_by_message.insert(entry, turn.to_string());
    }

    /// Prefer the exact native turn before the fallback entry-to-turn link.
    pub(crate) fn turn_id(&self, entry: EntryId) -> Option<&str> {
        if let Some(native) = self.native_user(entry) {
            return Some(&native.turn_id);
        }
        self.turn_ids_by_message.get(&entry).map(String::as_str)
    }

    pub(in crate::session) fn discard_pending_history(&mut self, text: &str) -> bool {
        if self
            .input_history
            .last()
            .is_some_and(|pending| pending == text)
            && self.input_history_entry_ids.last() == Some(&None)
        {
            self.input_history.pop();
            self.prompt_ids.pop();
            self.input_history_entry_ids.pop();
            return true;
        }
        false
    }

    pub(in crate::session) fn record_history(
        &mut self,
        text: &str,
        entry: Option<EntryId>,
        ids: &mut EntryIds,
    ) -> bool {
        if text.is_empty() {
            return true;
        }

        if let Some(entry) = entry {
            if !self.recorded.insert(entry) {
                return false;
            }
            if let (Some(last_text), Some(last_entry)) = (
                self.input_history.last(),
                self.input_history_entry_ids.last_mut(),
            ) {
                if *last_text == text && last_entry.is_none() {
                    *last_entry = Some(entry);
                    return true;
                }
            }
        }

        self.prompt_ids
            .push(entry.unwrap_or_else(|| ids.allocate()));
        self.input_history.push(text.to_string());
        self.input_history_entry_ids.push(entry);
        true
    }

    #[cfg(test)]
    pub(in crate::session) fn retain_entries(&mut self, transcript: &Transcript) {
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
    pub(crate) fn message_ids(&self) -> &[Option<EntryId>] {
        &self.input_history_entry_ids
    }
}

#[cfg(test)]
impl PromptHistory {
    pub(crate) fn assert_invariants(&self, transcript: &Transcript) {
        assert_eq!(self.input_history.len(), self.input_history_entry_ids.len());
        assert_eq!(self.input_history.len(), self.prompt_ids.len());
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
