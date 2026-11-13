//! Transcript item identity, lifecycle, and streamed text updates.

use crate::session::entry_key::BackendEntryKey;

use super::state::AppState;
use crate::session::models::MessageKind;
use crate::session::models::Role;

impl AppState {
    // --- Item Identity ---

    /// Scope native work by backend, conversation, turn and a distinct display part.
    pub(super) fn item_key(&self, item_id: &str) -> BackendEntryKey {
        self.session.backend_entry_key(item_id)
    }

    /// Keep native IDs unchanged while selecting a separate display part.
    #[cfg(test)]
    pub(super) fn with_item_part<R>(
        &mut self,
        item: &str,
        part: usize,
        apply: impl FnOnce(&mut Self, &str) -> R,
    ) -> R {
        let previous = self.session.replace_codex_item_part(part);
        let result = apply(self, item);
        self.session.replace_codex_item_part(previous);
        result
    }

    /// Resolve an id only at the boundary of an in-process store operation.
    #[cfg(test)]
    pub(super) fn item_message_index(&self, item_id: &str) -> Option<usize> {
        self.session.item_message_index(&self.item_key(item_id))
    }

    #[cfg(test)]
    pub(super) fn has_item_mapping(&self, item_id: &str) -> bool {
        self.session.has_item_mapping(&self.item_key(item_id))
    }

    #[cfg(test)]
    pub(super) fn put_item_mapping(&mut self, item_id: &str, idx: usize) {
        let key = self.item_key(item_id);
        self.session.put_item_mapping(&key, idx);
    }

    #[cfg(test)]
    pub(super) fn mark_item_completed(&mut self, item_id: &str) {
        let key = self.item_key(item_id);
        self.session.mark_item_completed(&key);
    }

    #[cfg(test)]
    pub(super) fn is_item_completed(&self, item_id: &str) -> bool {
        self.session.is_item_completed(&self.item_key(item_id))
    }

    #[cfg(test)]
    pub(super) fn ensure_item_placeholder(&mut self, item_id: &str, role: Role) -> bool {
        let key = self.item_key(item_id);
        let (result, update) = self.session.ensure_item_placeholder(&key, role);
        self.apply_session_update(update);
        result
    }

    // --- Completed Items ---

    pub(super) fn upsert_mapped_message(
        &mut self,
        item_id: &str,
        role: Role,
        text: String,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> bool {
        let key = self.item_key(item_id);
        let (result, update) = self
            .session
            .upsert_mapped_message(&key, role, text, kind, file_path);
        self.apply_session_update(update);
        result
    }

    // --- Streamed Items ---

    #[cfg(test)]
    pub(super) fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
        let key = self.item_key(item_id);
        let update = self.session.upsert_agent_delta(&key, delta);
        self.apply_session_update(update);
    }

    #[cfg(test)]
    pub(super) fn upsert_reasoning_summary_delta(&mut self, item_id: &str, delta: &str) {
        if self.is_item_completed(item_id) {
            return;
        }
        let key = self.item_key(item_id);
        let update = self.session.upsert_reasoning_delta(&key, delta);
        self.apply_session_update(update);
        let Some(idx) = self.item_message_index(item_id) else {
            return;
        };
        let update = self.session.normalize_reasoning_at(idx);
        self.apply_session_update(update);
    }

    #[cfg(test)]
    pub(super) fn upsert_command_output_delta(&mut self, item_id: &str, delta: &str) {
        let key = self.item_key(item_id);
        let update = self.session.upsert_command_output_delta(&key, delta);
        self.apply_session_update(update);
    }
}
