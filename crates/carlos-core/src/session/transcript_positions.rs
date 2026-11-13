//! Resolve ordered ids directly and retain a map only for entries outside that order.

use std::collections::HashMap;

use super::super::entry_id::EntryId;
use crate::session::models::Message;

#[derive(Default)]
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
/// Resolve live entry ids without hashing the entries that follow their stored order.
pub(super) struct EntryPositions {
    first: Option<EntryId>,
    sparse: HashMap<EntryId, usize>,
    reserve: usize,
}

impl EntryPositions {
    /// Verify a direct counter offset before falling back to exact mapped ids.
    pub(super) fn find(&self, entries: &[Message], id: EntryId) -> Option<usize> {
        if let Some(index) = self.first.and_then(|first| id.offset_from(first)) {
            if entries.get(index).is_some_and(|entry| entry.id == id) {
                return Some(index);
            }
        }
        self.sparse.get(&id).copied()
    }

    /// Defer hash-bucket allocation until a reordered entry actually needs a map.
    pub(super) fn reserve(&mut self, count: usize) {
        self.reserve = self.reserve.max(count);
    }

    /// Index one newly adopted id; the transcript never inserts the same id twice.
    #[inline(always)]
    pub(super) fn insert(&mut self, id: EntryId, index: usize) {
        let first = *self.first.get_or_insert(id);
        if id.offset_from(first) == Some(index) {
            return;
        }
        if self.reserve > 0 {
            self.sparse.reserve(std::mem::take(&mut self.reserve));
        }
        self.sparse.insert(id, index);
    }

    /// Retire the mapped address, if this entry needed one.
    pub(super) fn remove(&mut self, id: &EntryId) {
        self.sparse.remove(id);
    }

    /// Trim the deferred reserve and retire the seed when no rows remain.
    pub(super) fn finish_truncate(&mut self, remaining: usize) {
        self.reserve = self.reserve.min(remaining);
        if remaining == 0 {
            self.first = None;
            self.reserve = 0;
            self.sparse.clear();
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Rebuild the test fixture after inserting a row in the middle.
    pub(super) fn rebuild(&mut self, entries: &[Message]) {
        *self = Self::default();
        self.reserve(entries.len());
        for (index, row) in entries.iter().enumerate() {
            self.insert(row.id, index);
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    /// Assert that every stored exception still names its exact live row.
    pub(super) fn assert_invariants(&self, entries: &[Message]) {
        for (id, index) in &self.sparse {
            assert_eq!(entries.get(*index).map(|row| row.id), Some(*id));
        }
    }
}
