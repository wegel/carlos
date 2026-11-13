//! Bind native sources and retire cursors superseded by a newer translator snapshot.

use super::super::entry_id::EntryId;
use super::super::entry_key::{BackendEntryKey, NativeEntryKey};
use super::Transcript;

impl Transcript {
    /// Bind a key only to an entry that is still alive.
    pub(crate) fn bind_item(&mut self, key: BackendEntryKey, id: EntryId) {
        assert!(self.position(id).is_some(), "binding to a retired entry");
        if self.entry_id_for_item(&key) == Some(id) {
            return;
        }
        if let Some(previous) = self.item_to_entry_id.insert(key.clone(), id) {
            if let Some(aliases) = self.aliases.get_mut(&previous) {
                aliases.retain(|alias| *alias != key);
            }
        }
        let mut base = key.clone();
        base.part = 0;
        let count = self.part_counts.entry(base).or_default();
        *count = (*count).max(key.part + 1);
        if key.durable() {
            self.sources.insert(id, key.clone());
        }
        self.aliases.entry(id).or_default().push(key);
    }

    /// Borrow the native source before accepting a printable cursor alias.
    pub fn source(&self, id: EntryId) -> Option<&BackendEntryKey> {
        self.sources.get(&id)
    }

    /// Return the number of parts currently addressable by a cursor.
    pub(crate) fn part_count(&self, key: &BackendEntryKey) -> usize {
        let mut base = key.clone();
        base.part = 0;
        self.part_counts.get(&base).copied().unwrap_or(0)
    }

    /// Remove one cursor without retiring its native row or its other sources.
    pub(crate) fn unbind_item(&mut self, key: &BackendEntryKey) {
        if let Some(id) = self.item_to_entry_id.remove(key) {
            if let Some(aliases) = self.aliases.get_mut(&id) {
                aliases.retain(|alias| alias != key);
            }
        }
        let mut base = key.clone();
        base.part = 0;
        let mut count = self.part_count(&base);
        while count > 0 {
            base.part = count - 1;
            if self.item_to_entry_id.contains_key(&base) {
                break;
            }
            count -= 1;
        }
        base.part = 0;
        if count == 0 {
            self.part_counts.remove(&base);
        } else {
            self.part_counts.insert(base, count);
        }
    }

    /// Keep only the newest transient cursor for a native row and display part.
    pub(crate) fn retire_cursors(
        &mut self,
        id: EntryId,
        current: &BackendEntryKey,
    ) -> Vec<BackendEntryKey> {
        let retired: Vec<_> = self
            .aliases
            .get(&id)
            .into_iter()
            .flatten()
            .filter(|key| matches!(key.item, NativeEntryKey::ClaudeTransient(_)) && *key != current)
            .cloned()
            .collect();
        for key in &retired {
            self.unbind_item(key);
        }
        retired
    }
}
