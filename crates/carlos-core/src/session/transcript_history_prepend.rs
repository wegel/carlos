//! Move an older prefix into the store without cloning or renumbering existing rows.

use super::Transcript;
use crate::session::entry_id::EntryId;
use crate::session::history_identity::{HistoryEntry, HistoryMode};

impl Transcript {
    pub(in crate::session) fn prepend_history(
        &mut self,
        drafts: Vec<HistoryEntry>,
    ) -> Vec<EntryId> {
        let mut prefix = Transcript::new();
        std::mem::swap(&mut prefix.ids, &mut self.ids);
        let ids = prefix.replace_history(drafts, HistoryMode::SameThread);
        std::mem::swap(&mut prefix.ids, &mut self.ids);
        let count = prefix.entries.len();
        prefix
            .active
            .extend(self.active.iter().map(|index| index + count));
        self.active = prefix.active;
        prefix.entries.append(&mut self.entries);
        self.entries = prefix.entries;
        self.positions.rebuild(&self.entries);
        self.item_to_entry_id.extend(prefix.item_to_entry_id);
        self.sources.extend(prefix.sources);
        self.aliases.extend(prefix.aliases);
        self.part_counts.extend(prefix.part_counts);
        self.ambiguous_sources.extend(prefix.ambiguous_sources);
        self.output_revisions.extend(prefix.output_revisions);
        self.turn_diff_to_entry_id
            .extend(prefix.turn_diff_to_entry_id);
        ids
    }
}
