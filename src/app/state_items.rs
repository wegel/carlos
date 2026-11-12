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
            conversation: self.session.entry_conversation().clone(),
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
        self.session.item_message_index(&self.item_key(item_id))
    }

    pub(super) fn has_item_mapping(&self, item_id: &str) -> bool {
        self.session.has_item_mapping(&self.item_key(item_id))
    }

    pub(super) fn put_item_mapping(&mut self, item_id: &str, idx: usize) {
        let key = self.item_key(item_id);
        self.session.put_item_mapping(&key, idx);
    }

    pub(super) fn mark_item_completed(&mut self, item_id: &str) {
        let key = self.item_key(item_id);
        self.session.mark_item_completed(&key);
    }

    pub(super) fn is_item_completed(&self, item_id: &str) -> bool {
        self.session.is_item_completed(&self.item_key(item_id))
    }

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

    pub(super) fn upsert_agent_delta(&mut self, item_id: &str, delta: &str) {
        let key = self.item_key(item_id);
        let update = self.session.upsert_agent_delta(&key, delta);
        self.apply_session_update(update);
    }

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

    pub(super) fn upsert_command_output_delta(&mut self, item_id: &str, delta: &str) {
        let key = self.item_key(item_id);
        let update = self.session.upsert_command_output_delta(&key, delta);
        self.apply_session_update(update);
    }
}

#[path = "state_item_sources.rs"]
mod sources;
