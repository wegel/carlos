//! Reject late older pages only when a replacement or an unseen row change makes them unsafe.

use std::collections::{HashMap, HashSet, VecDeque};

use carlos_proto::ids::{EntryId, Sequence};
use carlos_proto::session::History;

use crate::replica::{ChangeSet, Replica};
use crate::ClientError;

const MAX_DIRTY_ROWS: usize = 1024;

pub(crate) struct HistoryFence {
    floor: Sequence,
    unknown: HashMap<EntryId, Sequence>,
    recent: VecDeque<(EntryId, Sequence)>,
}

impl HistoryFence {
    pub fn new(floor: Sequence) -> Self {
        Self {
            floor,
            unknown: HashMap::new(),
            recent: VecDeque::new(),
        }
    }

    pub fn changed_unknown(&mut self, entry: EntryId, seq: Sequence) {
        self.unknown.insert(entry.clone(), seq);
        self.recent.push_back((entry, seq));
        while self.recent.len() > MAX_DIRTY_ROWS {
            let (entry, seq) = self.recent.pop_front().expect("nonempty history fence");
            self.floor = self.floor.max(seq);
            if self.unknown.get(&entry) == Some(&seq) {
                self.unknown.remove(&entry);
            }
        }
    }

    fn permits(&self, history: &History) -> bool {
        history.seq >= self.floor
            && history.entries.iter().all(|entry| {
                self.unknown
                    .get(&entry.id)
                    .is_none_or(|seq| *seq <= history.seq)
            })
    }
}

impl Replica {
    /// Prepend a contiguous older page; never overwrite newer facts or splice across a missing anchor.
    pub fn merge_history(
        &mut self,
        before: &EntryId,
        page: History,
    ) -> Result<ChangeSet, ClientError> {
        if self.stale
            || page.seq > self.seq
            || !self.history.permits(&page)
            || self.rows.front().map(|entry| &entry.id) != Some(before)
        {
            return Err(ClientError::StaleHistory);
        }
        let mut unique = HashSet::new();
        for entry in &page.entries {
            if !unique.insert(entry.id.clone())
                || self.rows.get(&entry.id).is_some()
                || self.approvals.contains(&entry.id)
            {
                return Err(ClientError::DuplicateEntry(entry.id.clone()));
            }
        }
        if page.entries.is_empty() && page.has_older {
            return Err(ClientError::StaleHistory);
        }
        let mut changed = ChangeSet {
            entries: page.entries.iter().map(|entry| entry.id.clone()).collect(),
            ..Default::default()
        };
        self.rows.prepend(page.entries)?;
        self.older = page.has_older;
        changed.removed_back = self.rows.bound(false)?;
        self.newer |= changed.removed_back > 0;
        Ok(changed)
    }
}
