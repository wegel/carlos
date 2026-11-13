//! Owns ordered transcript entries, their ids, and live backend lookups.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::Deref;

use self::positions::EntryPositions;
use super::entry_id::{EntryId, EntryIds};
use super::entry_key::BackendEntryKey;
#[cfg(any(test, feature = "test-support"))]
use super::history_identity::HistoryMode;
use super::models::{EntryDraft, Message, MessageKind, Role, ToolOutputDetails};

/// Stores source entries contiguously; clients may borrow the slice but cannot mutate it.
#[cfg_attr(any(test, feature = "test-support"), derive(Debug))]
pub struct Transcript {
    pub(super) entries: Vec<Message>,
    // These offsets stay inside the store; backend keys and callers retain entry ids.
    active: BTreeSet<usize>,
    positions: EntryPositions,
    pub(super) aliases: HashMap<EntryId, Vec<BackendEntryKey>>,
    pub(super) part_counts: HashMap<BackendEntryKey, usize>,
    pub(super) item_to_entry_id: HashMap<BackendEntryKey, EntryId>,
    pub(super) turn_diff_to_entry_id: HashMap<String, EntryId>,
    pub(super) ambiguous_sources: HashSet<BackendEntryKey>,
    pub(super) sources: HashMap<EntryId, BackendEntryKey>,
    pub(super) ids: EntryIds,
}

impl Transcript {
    /// Create an empty transcript in a fresh Carlos namespace.
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            active: BTreeSet::new(),
            positions: EntryPositions::default(),
            aliases: HashMap::new(),
            part_counts: HashMap::new(),
            item_to_entry_id: HashMap::new(),
            turn_diff_to_entry_id: HashMap::new(),
            sources: HashMap::new(),
            ambiguous_sources: HashSet::new(),
            ids: EntryIds::new(),
        }
    }

    /// Borrow entries in display order without any per-row lookup.
    pub fn entries(&self) -> &[Message] {
        &self.entries
    }

    /// Resolve a live id once before using contiguous render caches.
    pub fn position(&self, id: EntryId) -> Option<usize> {
        self.positions.find(&self.entries, id)
    }

    /// Borrow a live entry; retired ids never resolve to a replacement row.
    pub fn entry(&self, id: EntryId) -> Option<&Message> {
        self.entries.get(self.position(id)?)
    }

    /// Find the entry bound to a native key.
    pub fn entry_id_for_item(&self, key: &BackendEntryKey) -> Option<EntryId> {
        self.item_to_entry_id.get(key).copied()
    }

    /// Borrow live keys for event handling, outside the row draw loop.
    #[cfg(any(test, feature = "test-support"))]
    pub fn items(&self) -> &HashMap<BackendEntryKey, EntryId> {
        &self.item_to_entry_id
    }

    /// Adopt one draft and return its newly allocated id.
    #[inline(always)]
    pub fn append(&mut self, draft: EntryDraft) -> EntryId {
        let id = self.ids.allocate();
        self.insert(id, draft);
        id
    }

    /// Reserve all row storage once when a complete draft already supplies its size.
    pub(super) fn reserve_entries(&mut self, count: usize) {
        self.entries.reserve(count);
        self.positions.reserve(count);
    }

    /// Insert an id selected by this store during append or history adoption.
    #[inline(always)]
    pub(super) fn insert(&mut self, id: EntryId, draft: EntryDraft) {
        self.positions.insert(id, self.entries.len());
        self.entries.push(Message {
            id,
            role: draft.role,
            text: draft.text,
            kind: draft.kind,
            file_path: draft.file_path,
            tool_output: draft.tool_output,
        });
    }

    /// Allocate a card id from the same allocator without inserting a transcript row.
    pub(crate) fn allocate_card(&mut self) -> EntryId {
        self.ids.allocate()
    }

    /// Mark a mapped entry active once, outside the render path.
    pub fn activate(&mut self, id: EntryId) {
        if let Some(index) = self.position(id) {
            self.active.insert(index);
        }
    }

    /// Finish the currently handled mapped row, preserving existing display-step rules.
    pub fn complete(&mut self, key: &BackendEntryKey) {
        if let Some(index) = self.entry_id_for_item(key).and_then(|id| self.position(id)) {
            self.active.remove(&index);
        }
    }

    /// Report whether a mapped source already reached completion.
    pub(crate) fn is_complete(&self, id: EntryId) -> bool {
        self.position(id)
            .is_some_and(|index| !self.active.contains(&index))
    }

    /// Visit only unfinished rows, from the latest stored position backwards.
    pub(crate) fn active_step(&self, start: usize) -> Option<usize> {
        self.active
            .range(start..)
            .rev()
            .copied()
            .find(|index| !self.entries[*index].text.is_empty())
    }

    /// Retire absent diff-part lookups while leaving their existing display rows intact.
    pub(crate) fn retain_parts(&mut self, key: &BackendEntryKey, count: usize) {
        let mut part_key = key.clone();
        part_key.part = 0;
        let previous = self.part_counts.get(&part_key).copied().unwrap_or(0);
        for part in count..previous {
            part_key.part = part;
            if let Some(id) = self.entry_id_for_item(&part_key) {
                self.unbind_entry(id);
            }
        }
    }

    fn unbind_entry(&mut self, id: EntryId) {
        for mut key in self.aliases.remove(&id).unwrap_or_default() {
            self.item_to_entry_id.remove(&key);
            let part = key.part;
            key.part = 0;
            if part == 0 {
                self.part_counts.remove(&key);
            } else if let Some(count) = self.part_counts.get_mut(&key) {
                *count = (*count).min(part);
            }
        }
        if let Some(key) = self.sources.remove(&id) {
            self.ambiguous_sources.remove(&key);
        }
        if let Some(index) = self.position(id) {
            self.active.remove(&index);
        }
    }

    /// Find a per-turn aggregate diff without exposing its storage position.
    pub fn turn_diff(&self, turn: &str) -> Option<EntryId> {
        self.turn_diff_to_entry_id.get(turn).copied()
    }

    /// Bind a per-turn aggregate diff to a live entry.
    pub(crate) fn bind_turn_diff(&mut self, turn: String, id: EntryId) {
        assert!(self.position(id).is_some());
        self.turn_diff_to_entry_id.insert(turn, id);
    }

    /// Start a new conversation while leaving external process-scoped cards alone.
    pub fn reset(&mut self) {
        if let Some(message) = self.entries.first() {
            self.truncate_before(message.id);
        }
        self.ids = EntryIds::new();
    }

    /// Retain the prefix strictly before this id, without rewinding the allocator.
    pub(crate) fn truncate_before(&mut self, id: EntryId) -> bool {
        let Some(position) = self.position(id) else {
            return false;
        };
        self.truncate(position);
        true
    }

    fn truncate(&mut self, position: usize) {
        self.active.split_off(&position);
        let removed = self.entries.split_off(position);
        for message in removed {
            self.unbind_entry(message.id);
            self.positions.remove(&message.id);
        }
        self.positions.finish_truncate(self.entries.len());
        let positions = &self.positions;
        let entries = &self.entries;
        self.turn_diff_to_entry_id
            .retain(|_, id| positions.find(entries, *id).is_some());
    }

    /// Change entry facts while retaining the allocated id.
    pub fn update(
        &mut self,
        index: usize,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        path: Option<String>,
    ) {
        if let Some(message) = self.entries.get_mut(index) {
            message.role = role;
            if let Some(text) = text {
                message.text = text;
            }
            message.kind = kind;
            message.file_path = path;
        }
    }

    /// Append a delta in constant time apart from growing the entry text.
    pub(crate) fn append_text(&mut self, index: usize, delta: &str, role: Option<Role>) {
        if let Some(message) = self.entries.get_mut(index) {
            if let Some(role) = role {
                message.role = role;
            }
            if message.kind != MessageKind::Plain {
                message.kind = MessageKind::Plain;
                message.file_path = None;
                message.text.clear();
            }
            message.text.push_str(delta);
        }
    }

    /// Attach source-path facts without changing identity.
    pub(crate) fn set_path(&mut self, index: usize, path: Option<String>) {
        self.entries[index].file_path = path;
    }

    /// Update a prompt's native steer classification.
    pub(crate) fn set_kind(&mut self, index: usize, kind: MessageKind) {
        self.entries[index].kind = kind;
    }

    /// Attach a tool label and failure state to its original entry.
    pub(crate) fn set_tool(&mut self, index: usize, details: ToolOutputDetails) {
        self.entries[index].tool_output = Some(details);
    }

    /// Change only the label of an existing tool output.
    pub(crate) fn set_label(
        &mut self,
        id: EntryId,
        label: String,
        generated: bool,
    ) -> Option<usize> {
        let index = self.position(id)?;
        let tool = self.entries[index].tool_output.as_mut()?;
        tool.summary = label;
        tool.generated = generated;
        Some(index)
    }

    /// Adopt unkeyed drafts through the same allocator as live messages.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn adopt_unkeyed(&mut self, drafts: Vec<EntryDraft>, mode: HistoryMode) {
        let mut adopted = Transcript::new();
        if matches!(mode, HistoryMode::SameThread) {
            std::mem::swap(&mut adopted.ids, &mut self.ids);
        }
        adopted.reserve_entries(drafts.len());
        for draft in drafts {
            adopted.append(draft);
        }
        *self = adopted;
    }
}

impl Default for Transcript {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Transcript {
    type Target = [Message];
    fn deref(&self) -> &Self::Target {
        self.entries()
    }
}

#[cfg(test)]
#[path = "transcript_identity_tests.rs"]
mod identity_tests;

#[path = "transcript_aliases.rs"]
mod aliases;
#[cfg(any(test, feature = "test-support"))]
#[path = "transcript_invariants.rs"]
mod invariants;

#[path = "transcript_positions.rs"]
mod positions;

#[cfg(test)]
#[path = "transcript_position_tests.rs"]
mod position_tests;

#[cfg(test)]
#[path = "transcript_active_tests.rs"]
mod active_tests;
