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

    #[cfg(test)]
    pub(super) fn has_item_mapping(&self, item_id: &str) -> bool {
        self.session.has_item_mapping(&self.item_key(item_id))
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
}
