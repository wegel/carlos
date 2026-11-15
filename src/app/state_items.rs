//! Transcript item identity, lifecycle, and streamed text updates.

use carlos_core::session::entry_key::BackendEntryKey;
use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;

use super::state::AppState;

impl AppState {
    // --- Item Identity ---

    /// Scope native work by backend, conversation, turn and a distinct display part.
    pub(super) fn item_key(&self, item_id: &str) -> BackendEntryKey {
        self.session.backend_entry_key(item_id)
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
    pub(super) fn is_item_completed(&self, item_id: &str) -> bool {
        self.session.is_item_completed(&self.item_key(item_id))
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
        let result = self
            .session
            .upsert_mapped_message(&key, role, text, kind, file_path);
        self.consume_core_events();
        result
    }

    // --- Streamed Items ---

    #[cfg(test)]
    pub(super) fn upsert_reasoning_summary_delta(&mut self, item_id: &str, delta: &str) {
        if self.is_item_completed(item_id) {
            return;
        }
        let key = self.item_key(item_id);
        self.session.upsert_reasoning_delta(&key, delta);
        self.consume_core_events();
        let Some(idx) = self.item_message_index(item_id) else {
            return;
        };
        self.session.normalize_reasoning_at(idx);
        self.consume_core_events();
    }
}
