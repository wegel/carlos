//! Accept plain store writes from app bridges while keeping the store private.

use super::entry_id::EntryId;
use super::entry_key::BackendEntryKey;
#[cfg(test)]
use super::history_identity::HistoryEntry;
use super::history_identity::HistoryMode;
use super::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};
use super::state::SessionState;

impl SessionState {
    /// Append a draft and assign its transcript id.
    pub(crate) fn transcript_append(&mut self, draft: EntryDraft) -> EntryId {
        self.messages.append(draft)
    }

    /// Allocate a card id without appending a transcript row.
    pub(crate) fn transcript_allocate_card(&mut self) -> EntryId {
        self.messages.allocate_card()
    }

    /// Remove source parts beyond the supplied count.
    pub(crate) fn transcript_retain_parts(&mut self, key: &BackendEntryKey, count: usize) {
        self.messages.retain_parts(key, count)
    }

    /// Clear the transcript and its source indexes.
    #[cfg(test)]
    pub(crate) fn transcript_reset(&mut self) {
        self.messages.reset()
    }

    /// Replace the row’s role, text, kind and file path.
    pub(crate) fn transcript_update(
        &mut self,
        index: usize,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        path: Option<String>,
    ) {
        self.messages.update(index, role, text, kind, path)
    }

    /// Append text to a row and optionally change its role.
    pub(crate) fn transcript_append_text(&mut self, index: usize, delta: &str, role: Option<Role>) {
        self.messages.append_text(index, delta, role)
    }

    /// Set or clear the file path for a row.
    pub(crate) fn transcript_set_path(&mut self, index: usize, path: Option<String>) {
        self.messages.set_path(index, path)
    }

    /// Change how a row’s contents are classified.
    pub(crate) fn transcript_set_kind(&mut self, index: usize, kind: MessageKind) {
        self.messages.set_kind(index, kind)
    }

    /// Attach the tool’s output details to a row.
    pub(crate) fn transcript_set_tool(&mut self, index: usize, details: ToolOutputDetails) {
        self.messages.set_tool(index, details)
    }

    #[cfg(test)]
    /// Set a tool label and return the affected row index.
    pub(crate) fn transcript_set_label(
        &mut self,
        id: EntryId,
        label: String,
        generated: bool,
    ) -> Option<usize> {
        self.messages.set_label(id, label, generated)
    }

    /// Adopt history drafts that have no backend source keys.
    pub(crate) fn transcript_adopt_unkeyed(&mut self, drafts: Vec<EntryDraft>, mode: HistoryMode) {
        self.messages.adopt_unkeyed(drafts, mode)
    }

    #[cfg(test)]
    /// Keep direct store-replacement fixtures separate from the full history controller.
    pub(crate) fn transcript_replace_history(
        &mut self,
        drafts: Vec<HistoryEntry>,
        mode: HistoryMode,
    ) -> Vec<EntryId> {
        self.messages.replace_history(drafts, mode)
    }
}
