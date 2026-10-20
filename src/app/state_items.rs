//! Transcript item identity, lifecycle, and streamed text updates.

use super::models::{MessageKind, Role};
use super::state::AppState;

impl AppState {
    // --- Item Identity ---

    pub(super) fn has_item_mapping(&self, item_id: &str) -> bool {
        self.item_to_message_index.contains_key(item_id)
    }

    pub(super) fn put_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.item_to_message_index.insert(item_id.to_string(), idx);
    }

    pub(super) fn mark_item_completed(&mut self, item_id: &str) {
        self.completed_item_ids.insert(item_id.to_string());
    }

    pub(super) fn is_item_completed(&self, item_id: &str) -> bool {
        self.completed_item_ids.contains(item_id)
    }

    pub(super) fn ensure_item_placeholder(&mut self, item_id: &str, role: Role) -> bool {
        if self.has_item_mapping(item_id) {
            return false;
        }
        let idx = self.append_message(role, String::new());
        self.put_item_mapping(item_id, idx);
        true
    }

    // --- Completed Items ---

    pub(super) fn update_mapped_message(
        &mut self,
        item_id: &str,
        role: Role,
        text: Option<String>,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> bool {
        let Some(idx) = self.item_to_message_index.get(item_id).copied() else {
            return false;
        };
        self.update_message_at_index(idx, role, text, kind, file_path);
        true
    }

    pub(super) fn upsert_mapped_message(
        &mut self,
        item_id: &str,
        role: Role,
        text: String,
        kind: MessageKind,
        file_path: Option<String>,
    ) -> bool {
        if self.update_mapped_message(item_id, role, Some(text.clone()), kind, file_path.clone()) {
            return false;
        }

        let idx = if kind == MessageKind::Diff {
            self.append_diff_message(role, file_path, text)
        } else {
            self.append_message(role, text)
        };
        self.put_item_mapping(item_id, idx);
        true
    }

    // --- Streamed Items ---

    pub(super) fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
        self.upsert_text_delta(item_id, Role::Assistant, delta, false);
    }

    pub(super) fn upsert_reasoning_summary_delta(&mut self, item_id: &str, delta: &str) {
        if self.is_item_completed(item_id) {
            return;
        }
        self.upsert_text_delta(item_id, Role::Reasoning, delta, false);
        let Some(idx) = self.item_to_message_index.get(item_id).copied() else {
            return;
        };
        if let Some(message) = self.messages.get_mut(idx) {
            message.text =
                super::state_transcript::normalize_reasoning_summary_stream(&message.text);
            self.mark_transcript_dirty_from(idx);
        }
    }

    pub(super) fn upsert_command_output_delta(&mut self, item_id: &str, delta: &str) {
        self.upsert_text_delta(item_id, Role::ToolOutput, delta, true);
    }

    fn upsert_text_delta(&mut self, item_id: &str, role: Role, delta: &str, replace_role: bool) {
        if self.is_item_completed(item_id) {
            return;
        }
        if let Some(idx) = self.item_to_message_index.get(item_id).copied() {
            if let Some(message) = self.messages.get_mut(idx) {
                if replace_role {
                    message.role = role;
                }
                if message.kind != MessageKind::Plain {
                    message.kind = MessageKind::Plain;
                    message.file_path = None;
                    message.text.clear();
                }
                message.text.push_str(delta);
                self.mark_transcript_dirty_from(idx);
            }
            return;
        }

        let idx = self.append_message(role, delta);
        self.put_item_mapping(item_id, idx);
    }
}
