//! Accept plain store writes from app bridges while keeping the store private.

use super::entry_id::EntryId;
use super::entry_key::BackendEntryKey;
#[cfg(test)]
use super::history_identity::HistoryEntry;
use super::history_identity::HistoryMode;
use super::models::{EntryDraft, MessageKind, Role, ToolOutputDetails};
use super::state::SessionState;

impl SessionState {
    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_append(&mut self, draft: EntryDraft) -> EntryId {
        let id = self.messages.append(draft);
        self.observe_turn_append(self.messages.len() - 1);
        id
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_allocate_card(&mut self) -> EntryId {
        self.messages.allocate_card()
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_retain_parts(&mut self, key: &BackendEntryKey, count: usize) {
        self.messages.retain_parts(key, count)
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    #[cfg(test)]
    pub(crate) fn transcript_reset(&mut self) {
        self.messages.reset()
    }

    /// Apply a plain store command; the app retains its existing redraw order.
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

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_append_text(&mut self, index: usize, delta: &str, role: Option<Role>) {
        self.messages.append_text(index, delta, role)
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_set_path(&mut self, index: usize, path: Option<String>) {
        self.messages.set_path(index, path)
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_set_kind(&mut self, index: usize, kind: MessageKind) {
        self.messages.set_kind(index, kind)
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_set_tool(&mut self, index: usize, details: ToolOutputDetails) {
        self.messages.set_tool(index, details)
    }

    /// Apply a plain store command; the app retains its existing redraw order.
    pub(crate) fn transcript_set_label(
        &mut self,
        id: EntryId,
        label: String,
        generated: bool,
    ) -> Option<usize> {
        self.messages.set_label(id, label, generated)
    }

    /// Apply a plain store command; the app retains its existing redraw order.
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
