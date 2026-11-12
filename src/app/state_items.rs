//! Transcript item identity, lifecycle, and streamed text updates.

use crate::backend::BackendKind;
use crate::session::entry_key::{BackendEntryKey, NativeEntryKey};

use super::models::{MessageKind, Role};
use super::state::AppState;

impl AppState {
    // --- Item Identity ---

    /// Scope native work by backend, conversation, turn and a distinct display part.
    pub(super) fn item_key(&self, item_id: &str) -> BackendEntryKey {
        let backend = if self.codex.enabled {
            BackendKind::Codex
        } else if self.claude.enabled {
            BackendKind::Claude
        } else {
            BackendKind::Grok
        };
        let item = if backend == BackendKind::Claude {
            item_id
                .strip_prefix("claude-prompt:")
                .map(|id| NativeEntryKey::ClaudePrompt(id.into()))
                .unwrap_or_else(|| NativeEntryKey::ClaudeTransient(item_id.into()))
        } else {
            NativeEntryKey::Item(item_id.into())
        };
        BackendEntryKey {
            backend,
            conversation: self.entry_conversation.clone(),
            turn: self.codex.item_turn.clone().filter(|_| self.codex.enabled),
            item,
            part: self.codex.item_part,
        }
    }

    /// Keep native IDs unchanged while selecting a separate display part.
    pub(super) fn with_item_part<R>(
        &mut self,
        item: &str,
        part: usize,
        apply: impl FnOnce(&mut Self, &str) -> R,
    ) -> R {
        let previous = std::mem::replace(&mut self.codex.item_part, part);
        let result = apply(self, item);
        self.codex.item_part = previous;
        result
    }

    /// Resolve an id only at the boundary of an in-process store operation.
    pub(super) fn item_message_index(&self, item_id: &str) -> Option<usize> {
        self.messages
            .position(self.messages.entry_id_for_item(&self.item_key(item_id))?)
    }

    pub(super) fn has_item_mapping(&self, item_id: &str) -> bool {
        self.item_message_index(item_id).is_some()
    }

    pub(super) fn put_item_mapping(&mut self, item_id: &str, idx: usize) {
        self.messages.activate(self.messages[idx].id);
        self.messages
            .bind_item(self.item_key(item_id), self.messages[idx].id);
    }

    pub(super) fn mark_item_completed(&mut self, item_id: &str) {
        self.messages.complete(&self.item_key(item_id));
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
            self.messages.set_path(idx, file_path);
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
        let text =
            super::state_transcript::normalize_reasoning_summary_stream(&self.messages[idx].text);
        let message = &self.messages[idx];
        self.messages.update(
            idx,
            message.role,
            Some(text),
            message.kind,
            message.file_path.clone(),
        );
        self.mark_transcript_dirty_from(idx);
    }

    pub(super) fn upsert_command_output_delta(&mut self, item_id: &str, delta: &str) {
        self.upsert_text_delta(item_id, Role::ToolOutput, delta, true);
    }

    fn upsert_text_delta(&mut self, item_id: &str, role: Role, delta: &str, replace_role: bool) {
        if self.is_item_completed(item_id) {
            return;
        }
        if let Some(idx) = self.item_message_index(item_id) {
            self.messages
                .append_text(idx, delta, replace_role.then_some(role));
            self.mark_transcript_dirty_from(idx);
            return;
        }

        let idx = self.append_message(role, delta);
        self.put_item_mapping(item_id, idx);
    }
}

#[path = "state_item_sources.rs"]
mod sources;
