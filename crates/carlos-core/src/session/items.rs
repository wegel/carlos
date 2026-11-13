//! Resolve native entry keys and apply complete or streamed row writes.

use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role};
use super::state::SessionState;

impl SessionState {
    /// Resolve a native key to its current row without storing a row number.
    pub fn item_message_index(&self, key: &BackendEntryKey) -> Option<usize> {
        self.messages
            .position(self.messages.entry_id_for_item(key)?)
    }

    /// Check whether a native key still addresses a live row.
    pub fn has_item_mapping(&self, key: &BackendEntryKey) -> bool {
        self.item_message_index(key).is_some()
    }

    /// Mark a row active and bind its native key to its entry id.
    pub fn put_item_mapping(&mut self, key: &BackendEntryKey, idx: usize) {
        self.messages.activate(self.messages[idx].id);
        self.messages.bind_item(key.clone(), self.messages[idx].id);
    }

    /// Record completion both on the entry and on its scoped native key.
    pub fn mark_item_completed(&mut self, key: &BackendEntryKey) {
        self.messages.complete(key);
        self.completed_item_ids.insert(key.clone());
    }

    /// Check whether this scoped native item already finished.
    pub fn is_item_completed(&self, key: &BackendEntryKey) -> bool {
        self.completed_item_ids.contains(key)
    }

    /// Append one empty row only when its native key has no live entry.
    pub fn ensure_item_placeholder(&mut self, key: &BackendEntryKey, role: Role) -> bool {
        if self.has_item_mapping(key) {
            return false;
        }
        let idx = self.append_message(role, String::new());
        self.put_item_mapping(key, idx);
        true
    }

    /// Replace a known row and report the preceding read label when it changes.
    pub fn update_mapped_message(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> bool {
        let Some(idx) = self.item_message_index(key) else {
            return false;
        };
        self.update_message_at_index(idx, role, text, kind, file_path);
        true
    }

    /// Update a known row or append and bind a new row.
    pub fn upsert_mapped_message(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
        text: String,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> bool {
        let found =
            self.update_mapped_message(key, role, Some(text.clone()), kind, file_path.clone());
        if found {
            return false;
        }

        let idx = if kind == MessageKind::Diff {
            self.append_diff_message(role, file_path, text)
        } else {
            let idx = self.append_message(role, text);
            self.messages.set_path(idx, file_path);
            idx
        };
        self.put_item_mapping(key, idx);
        true
    }

    /// Append streamed assistant text unless that native item already finished.
    pub fn upsert_agent_delta(&mut self, key: &BackendEntryKey, delta: &str) {
        self.upsert_text_delta(key, Role::Assistant, delta, false)
    }

    /// Append command output and change the row to the tool-output role.
    pub fn upsert_command_output_delta(&mut self, key: &BackendEntryKey, delta: &str) {
        self.upsert_text_delta(key, Role::ToolOutput, delta, true)
    }

    /// Append streamed text to a live row or create its first row.
    pub(super) fn upsert_text_delta(
        &mut self,
        key: &BackendEntryKey,
        role: Role,
        delta: &str,
        replace_role: bool,
    ) {
        if self.is_item_completed(key) {
            return;
        }
        if let Some(idx) = self.item_message_index(key) {
            self.messages
                .append_text(idx, delta, replace_role.then_some(role));
            return;
        }

        let idx = self.append_message(role, delta);
        self.put_item_mapping(key, idx);
    }
}
