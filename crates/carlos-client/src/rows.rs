//! Keep a contiguous bounded client window and constant-time stable-id lookups while paging.

use std::collections::{HashMap, HashSet, VecDeque};

use carlos_proto::ids::EntryId;
use carlos_proto::session::{Entry, EntryKind};

use crate::ClientError;

pub(crate) const MAX_ROWS: usize = 100_000;
pub(crate) const MAX_ROW_BYTES: usize = 32 * 1024 * 1024;

impl crate::Replica {
    /// Count chronological rows without walking their source strings.
    pub fn entry_count(&self) -> usize {
        self.rows.len()
    }
    /// Fold only a call whose single proven native result is in this client window.
    pub fn has_tool_result(&self, call: &EntryId) -> bool {
        self.rows.has_tool_result(call)
    }
}

pub(crate) struct Rows {
    entries: VecDeque<Entry>,
    positions: HashMap<EntryId, i128>,
    first: i128,
    bytes: usize,
    results: HashMap<EntryId, HashSet<EntryId>>,
}

impl Rows {
    pub(crate) fn retained_bytes(&self) -> usize {
        self.bytes
    }
    pub fn new(entries: Vec<Entry>) -> Result<Self, ClientError> {
        let mut rows = Self {
            entries: VecDeque::new(),
            positions: HashMap::new(),
            first: 0,
            bytes: 0,
            results: HashMap::new(),
        };
        for entry in entries {
            rows.push(entry)?;
        }
        Ok(rows)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn at(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index)
    }

    pub fn position(&self, id: &EntryId) -> Option<usize> {
        usize::try_from(*self.positions.get(id)? - self.first).ok()
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

    pub(crate) fn has_tool_result(&self, id: &EntryId) -> bool {
        self.get(id)
            .is_some_and(|entry| entry.kind == EntryKind::ToolCall)
            && self
                .results
                .get(id)
                .is_some_and(|results| results.len() == 1)
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
        self.index_result(&entry);
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
            self.index_result(&entry);
            self.entries.push_front(entry);
        }
        Ok(())
    }

    pub fn changed(&mut self, old_weight: usize, old_call: Option<&EntryId>, entry: &EntryId) {
        let call = self.get(entry).and_then(result_call).cloned();
        if old_call != call.as_ref() {
            self.unindex_result(old_call, entry);
            if let Some(call) = call {
                self.results.entry(call).or_default().insert(entry.clone());
            }
        }
        self.bytes = self
            .bytes
            .saturating_sub(old_weight)
            .saturating_add(self.get(entry).map_or(0, weight));
    }

    pub fn bound(
        &mut self,
        oldest: bool,
        changed: &mut Vec<EntryId>,
    ) -> Result<usize, ClientError> {
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
            changed.extend(
                result_call(&entry)
                    .filter(|call| self.get(call).is_some())
                    .cloned(),
            );
            self.unindex_result(result_call(&entry), &entry.id);
            removed += 1;
        }
        Ok(removed)
    }

    fn index_result(&mut self, entry: &Entry) {
        if let Some(call) = result_call(entry) {
            self.results
                .entry(call.clone())
                .or_default()
                .insert(entry.id.clone());
        }
    }

    fn unindex_result(&mut self, call: Option<&EntryId>, result: &EntryId) {
        if let Some(call) = call {
            if let Some(results) = self.results.get_mut(call) {
                results.remove(result);
                if results.is_empty() {
                    self.results.remove(call);
                }
            }
        }
    }
}

pub(crate) fn result_call(entry: &Entry) -> Option<&EntryId> {
    (entry.kind == EntryKind::Tool).then_some(())?;
    entry
        .tool
        .as_ref()?
        .call
        .as_ref()
        .filter(|call| *call != &entry.id)
}

pub(crate) fn weight(entry: &Entry) -> usize {
    let tool = entry.tool.as_ref().map_or(0, |tool| {
        tool.label.len() + tool.summary.as_ref().map_or(0, String::len)
    });
    // This is an admitted source-data budget, not an exact allocator/heap bound.
    entry
        .text
        .len()
        .saturating_add(tool)
        .saturating_add(entry.file_path.as_ref().map_or(0, String::len))
        .saturating_add(256)
}
