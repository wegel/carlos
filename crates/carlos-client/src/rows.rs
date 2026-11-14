//! Keep a contiguous bounded client window and constant-time stable-id lookups while paging.

use std::collections::{HashMap, VecDeque};

use carlos_proto::ids::EntryId;
use carlos_proto::session::Entry;

use crate::ClientError;

pub(crate) const MAX_ROWS: usize = 100_000;
pub(crate) const MAX_ROW_BYTES: usize = 32 * 1024 * 1024;

pub(crate) struct Rows {
    entries: VecDeque<Entry>,
    positions: HashMap<EntryId, i128>,
    first: i128,
    bytes: usize,
}

impl Rows {
    pub fn new(entries: Vec<Entry>) -> Result<Self, ClientError> {
        let mut rows = Self {
            entries: VecDeque::new(),
            positions: HashMap::new(),
            first: 0,
            bytes: 0,
        };
        for entry in entries {
            rows.push(entry)?;
        }
        Ok(rows)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn get(&self, id: &EntryId) -> Option<&Entry> {
        self.entries
            .get(usize::try_from(*self.positions.get(id)? - self.first).ok()?)
    }

    pub fn get_mut(&mut self, id: &EntryId) -> Option<&mut Entry> {
        self.entries
            .get_mut(usize::try_from(*self.positions.get(id)? - self.first).ok()?)
    }

    pub fn front(&self) -> Option<&Entry> {
        self.entries.front()
    }

    pub fn push(&mut self, entry: Entry) -> Result<(), ClientError> {
        if self.positions.contains_key(&entry.id) {
            return Err(ClientError::DuplicateEntry(entry.id));
        }
        let position = self
            .first
            .checked_add(self.entries.len() as i128)
            .ok_or(ClientError::Capacity("client row positions exhausted"))?;
        self.bytes = self.bytes.saturating_add(weight(&entry));
        self.positions.insert(entry.id.clone(), position);
        self.entries.push_back(entry);
        Ok(())
    }

    pub fn prepend(&mut self, entries: Vec<Entry>) -> Result<(), ClientError> {
        for entry in entries.into_iter().rev() {
            if self.positions.contains_key(&entry.id) {
                return Err(ClientError::DuplicateEntry(entry.id));
            }
            self.first = self
                .first
                .checked_sub(1)
                .ok_or(ClientError::Capacity("client row positions exhausted"))?;
            self.bytes = self.bytes.saturating_add(weight(&entry));
            self.positions.insert(entry.id.clone(), self.first);
            self.entries.push_front(entry);
        }
        Ok(())
    }

    pub fn changed(&mut self, old_weight: usize, entry: &EntryId) {
        self.bytes = self
            .bytes
            .saturating_sub(old_weight)
            .saturating_add(self.get(entry).map_or(0, weight));
    }

    pub fn bound(&mut self, oldest: bool) -> Result<usize, ClientError> {
        let mut removed = 0;
        while self.entries.len() > MAX_ROWS || self.bytes > MAX_ROW_BYTES {
            let entry = if oldest {
                self.first = self
                    .first
                    .checked_add(1)
                    .ok_or(ClientError::Capacity("client row positions exhausted"))?;
                self.entries.pop_front()
            } else {
                self.entries.pop_back()
            };
            let Some(entry) = entry else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(weight(&entry));
            self.positions.remove(&entry.id);
            removed += 1;
        }
        Ok(removed)
    }
}

pub(crate) fn weight(entry: &Entry) -> usize {
    let tool = entry.tool.as_ref().map_or(0, |tool| {
        tool.label.len() + tool.summary.as_ref().map_or(0, String::len)
    });
    // This is an admitted source-data budget, not an exact allocator/heap bound.
    entry.text.len().saturating_add(tool).saturating_add(256)
}
