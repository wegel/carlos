//! Test every transcript map and the sizes of cursor bookkeeping.

use super::Transcript;
use crate::session::models::Message;

#[cfg(any(test, feature = "test-support"))]
impl Transcript {
    /// Assert invariants.
    pub fn assert_invariants(&self) {
        self.positions.assert_invariants(&self.entries);
        for index in &self.active {
            assert!(*index < self.entries.len());
        }
        for (index, row) in self.entries.iter().enumerate() {
            assert_eq!(self.position(row.id), Some(index));
        }
        for (key, id) in &self.item_to_entry_id {
            assert!(self.entry(*id).is_some());
            assert!(self
                .aliases
                .get(id)
                .is_some_and(|aliases| aliases.contains(key)));
        }
        for (id, aliases) in &self.aliases {
            assert!(self.entry(*id).is_some());
            for key in aliases {
                assert_eq!(self.entry_id_for_item(key), Some(*id));
            }
        }
        for id in self
            .sources
            .keys()
            .chain(self.turn_diff_to_entry_id.values())
        {
            assert!(self.entry(*id).is_some());
        }
        for (base, count) in &self.part_counts {
            let actual = self
                .item_to_entry_id
                .keys()
                .filter(|key| {
                    let mut key = (*key).clone();
                    key.part = 0;
                    key == *base
                })
                .map(|key| key.part + 1)
                .max();
            assert_eq!(actual, Some(*count));
        }
    }

    /// Insert for test.
    pub(crate) fn insert_for_test(&mut self, index: usize, message: Message) {
        let id = self.append(message.into());
        let row = self.entries.pop().expect("new row");
        self.active = self
            .active
            .iter()
            .map(|position| {
                if *position >= index {
                    position + 1
                } else {
                    *position
                }
            })
            .collect();
        self.entries.insert(index, row);
        self.positions.rebuild(&self.entries);
        assert_eq!(self.entries[index].id, id);
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Transcript {
    /// Alias sizes.
    pub fn alias_sizes(&self) -> (usize, usize, usize) {
        (
            self.item_to_entry_id.len(),
            self.aliases.values().map(Vec::len).sum(),
            self.part_counts.len(),
        )
    }
}
