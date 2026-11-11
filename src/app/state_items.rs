//! Transcript item identity, lifecycle, and streamed text updates.

use super::models::{MessageKind, Role};
use super::state::AppState;

impl AppState {
    // --- Item Identity ---

    /// Keep Codex item IDs local to their turn; raw IDs remain unchanged in native replies.
    pub(super) fn item_key(&self, item_id: &str) -> String {
        match self
            .codex
            .item_turn
            .as_deref()
            .filter(|_| self.codex.enabled)
        {
            Some(turn) => format!(
                "codex:{}",
                serde_json::json!([turn, item_id, self.codex.item_part])
            ),
            None => item_id.to_string(),
        }
    }

    /// Keep a display part distinct from a native ID containing the same printable suffix.
    pub(super) fn with_item_part<R>(
        &mut self,
        item: &str,
        part: usize,
        apply: impl FnOnce(&mut Self, &str) -> R,
    ) -> R {
        let previous = std::mem::replace(&mut self.codex.item_part, part);
        let key = if part == 0 || self.codex.enabled && self.codex.item_turn.is_some() {
            item.into()
        } else {
            format!("{item}#part-{part}")
        };
        let result = apply(self, &key);
        self.codex.item_part = previous;
        result
    }

    /// Resolve the currently handled native item's turn-qualified display row.
    pub(super) fn item_message_index(&self, item_id: &str) -> Option<usize> {
        self.item_to_message_index
            .get(&self.item_key(item_id))
            .copied()
    }

    pub(super) fn has_item_mapping(&self, item_id: &str) -> bool {
        self.item_message_index(item_id).is_some()
    }

    pub(super) fn put_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.item_to_message_index
            .insert(self.item_key(item_id), idx);
    }

    pub(super) fn mark_item_completed(&mut self, item_id: &str) {
        self.completed_item_ids.insert(self.item_key(item_id));
    }

    pub(super) fn is_item_completed(&self, item_id: &str) -> bool {
        self.completed_item_ids.contains(&self.item_key(item_id))
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
        let Some(idx) = self.item_message_index(item_id) else {
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
            let idx = self.append_message(role, text);
            self.messages[idx].file_path = file_path;
            idx
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
        let Some(idx) = self.item_message_index(item_id) else {
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
        if let Some(idx) = self.item_message_index(item_id) {
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
