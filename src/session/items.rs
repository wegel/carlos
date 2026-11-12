//! Resolve native entry keys and apply complete or streamed row writes.

use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role};
use super::state::SessionState;
use super::transcript_updates::SessionUpdate;

impl SessionState {
    /// Resolve a native key to its current row without storing a row number.
    pub(crate) fn item_message_index(&self, key: &BackendEntryKey) -> Option<usize> {
        self.messages
            .position(self.messages.entry_id_for_item(key)?)
    }

    /// Check whether a native key still addresses a live row.
    pub(crate) fn has_item_mapping(&self, key: &BackendEntryKey) -> bool {
        self.item_message_index(key).is_some()
    }

    /// Mark a row active and bind its native key to its entry id.
    pub(crate) fn put_item_mapping(&mut self, key: &BackendEntryKey, idx: usize) {
        self.messages.activate(self.messages[idx].id);
        self.messages.bind_item(key.clone(), self.messages[idx].id);
    }

    /// Record completion both on the entry and on its scoped native key.
    pub(crate) fn mark_item_completed(&mut self, key: &BackendEntryKey) {
        self.messages.complete(key);
        self.completed_item_ids.insert(key.clone());
    }

    /// Check whether this scoped native item already finished.
    pub(crate) fn is_item_completed(&self, key: &BackendEntryKey) -> bool {
        self.completed_item_ids.contains(key)
    }

    /// Append one empty row only when its native key has no live entry.
    pub(crate) fn ensure_item_placeholder(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
    ) -> (bool, SessionUpdate) {
        if self.has_item_mapping(key) {
            return (false, SessionUpdate::default());
        }
        let (idx, update) = self.append_message(role, String::new());
        self.put_item_mapping(key, idx);
        (true, update)
    }

    /// Replace a known row and report the preceding read label when it changes.
    pub(crate) fn update_mapped_message(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> (bool, SessionUpdate) {
        let Some(idx) = self.item_message_index(key) else {
            return (false, SessionUpdate::default());
        };
        (
            true,
            self.update_message_at_index(idx, role, text, kind, file_path),
        )
    }

    /// Update a known row or append and bind a new row.
    pub(crate) fn upsert_mapped_message(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
        text: String,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> (bool, SessionUpdate) {
        let (found, update) =
            self.update_mapped_message(key, role, Some(text.clone()), kind, file_path.clone());
        if found {
            return (false, update);
        }

        let (idx, update) = if kind == MessageKind::Diff {
            self.append_diff_message(role, file_path, text)
        } else {
            let (idx, update) = self.append_message(role, text);
            self.messages.set_path(idx, file_path);
            (idx, update)
        };
        self.put_item_mapping(key, idx);
        (true, update)
    }

    /// Append streamed assistant text unless that native item already finished.
    pub(crate) fn upsert_agent_delta(
        &mut self,
        key: &BackendEntryKey,
        delta: &str,
    ) -> SessionUpdate {
        self.upsert_text_delta(key, Role::Assistant, delta, false)
    }

    /// Append command output and change the row to the tool-output role.
    pub(crate) fn upsert_command_output_delta(
        &mut self,
        key: &BackendEntryKey,
        delta: &str,
    ) -> SessionUpdate {
        self.upsert_text_delta(key, Role::ToolOutput, delta, true)
    }

    /// Append streamed text to a live row or create its first row.
    pub(super) fn upsert_text_delta(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
        delta: &str,
        replace_role: bool,
    ) -> SessionUpdate {
        if self.is_item_completed(key) {
            return SessionUpdate::default();
        }
        if let Some(idx) = self.item_message_index(key) {
            self.messages
                .append_text(idx, delta, replace_role.then_some(role));
            return self.dirty_entry(idx);
        }

        let (idx, update) = self.append_message(role, delta);
        self.put_item_mapping(key, idx);
        update
    }
}
