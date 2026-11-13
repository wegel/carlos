//! Adopt complete history drafts by exact unique native keys, never by text or offset.

use std::collections::HashMap;

use super::entry_id::EntryId;
use super::entry_key::BackendEntryKey;
use super::models::EntryDraft;
use super::transcript::Transcript;

/// Select whether adoption preserves proven identities or starts a new conversation.
#[derive(Clone, Copy)]
pub enum HistoryMode {
    SameThread,
    NewConversation,
}

/// An unadopted entry and the native evidence available for matching it.
pub struct HistoryEntry {
    pub draft: EntryDraft,
    pub key: Option<BackendEntryKey>,
    pub occurrences: usize,
}

impl Transcript {
    /// Adopt checked drafts atomically and return each entry's adopted id in order.
    pub fn replace_history(
        &mut self,
        drafts: Vec<HistoryEntry>,
        mode: HistoryMode,
    ) -> Vec<EntryId> {
        let mut old = HashMap::new();
        if matches!(mode, HistoryMode::SameThread) {
            for (id, key) in &self.sources {
                old.entry(key.clone())
                    .and_modify(|value| *value = None)
                    .or_insert_with(|| (!self.ambiguous_sources.contains(key)).then_some(*id));
            }
        }
        let mut counts = HashMap::new();
        let mut rows_per_key = HashMap::new();
        for entry in &drafts {
            if let Some(key) = entry.key.as_ref().filter(|key| key.durable()) {
                *counts.entry(key.clone()).or_insert(0) += entry.occurrences;
                *rows_per_key.entry(key.clone()).or_insert(0) += 1;
            }
        }
        let mut adopted = Transcript::new();
        adopted.reserve_entries(drafts.len());
        if matches!(mode, HistoryMode::SameThread) {
            std::mem::swap(&mut adopted.ids, &mut self.ids);
        }
        let mut ids = Vec::with_capacity(drafts.len());
        for entry in drafts {
            let retained = entry
                .key
                .as_ref()
                .filter(|key| counts.get(*key) == Some(&1))
                .and_then(|key| old.get(key))
                .copied()
                .flatten();
            let id = retained.unwrap_or_else(|| adopted.ids.allocate());
            self.retain_output_revision(&mut adopted, id, &entry.draft);
            adopted.insert(id, entry.draft);
            if let Some(key) = entry.key {
                // Retain native evidence, including ambiguity, for the next reload.
                if key.durable() {
                    adopted.sources.insert(id, key.clone());
                }
                if counts.get(&key).is_some_and(|count| *count > 1) {
                    adopted.ambiguous_sources.insert(key.clone());
                }
                // A parser can merge repeated records into one display row. Keep
                // its live address, but never use ambiguous evidence to retain ids.
                if rows_per_key.get(&key).is_none_or(|count| *count == 1) {
                    adopted.activate(id);
                    adopted.bind_item(key, id);
                }
            }
            ids.push(id);
        }
        adopted.journal.published = adopted.len();
        *self = adopted;
        ids
    }
}
