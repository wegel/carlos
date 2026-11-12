//! Test every transcript map and the sizes of cursor bookkeeping.

use super::super::models::Message;
use super::Transcript;

#[cfg(test)]
impl Transcript {
    pub(crate) fn assert_invariants(&self) {
        assert_eq!(self.entries.len(), self.positions.len());
        assert_eq!(self.entries.len(), self.active.len());
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

    pub(crate) fn insert_for_test(&mut self, index: usize, message: Message) {
        let id = self.append(message.into());
        let row = self.entries.pop().expect("new row");
        self.active.pop();
        self.active.insert(index, false);
        self.entries.insert(index, row);
        for (position, row) in self.entries.iter().enumerate().skip(index) {
            self.positions.insert(row.id, position);
        }
        assert_eq!(self.entries[index].id, id);
    }
}

#[cfg(test)]
impl Transcript {
    pub(crate) fn alias_sizes(&self) -> (usize, usize, usize) {
        (
            self.item_to_entry_id.len(),
            self.aliases.values().map(Vec::len).sum(),
            self.part_counts.len(),
        )
    }
}
