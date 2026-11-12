//! Typed backend correlation keys; only keys backed by native evidence survive reloads.

use crate::backend::BackendKind;

/// Names a native entry and its display part inside one logical conversation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct BackendEntryKey {
    pub(crate) backend: BackendKind,
    pub(crate) conversation: String,
    pub(crate) turn: Option<String>,
    pub(crate) item: NativeEntryKey,
    pub(crate) part: usize,
}

/// Separates temporary Claude counters from native messages, tools and prompt UUIDs.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum NativeEntryKey {
    Item(String),
    ClaudeTransient(String),
    ClaudeAssistant { id: String, block: u64 },
    ClaudeTool { id: String, result: bool },
    ClaudePrompt(String),
}

impl BackendEntryKey {
    /// Report whether native evidence can identify this entry in a complete history.
    pub(crate) fn durable(&self) -> bool {
        match &self.item {
            NativeEntryKey::ClaudeTransient(_) => false,
            NativeEntryKey::Item(id)
            | NativeEntryKey::ClaudePrompt(id)
            | NativeEntryKey::ClaudeAssistant { id, .. }
            | NativeEntryKey::ClaudeTool { id, .. } => !id.is_empty(),
        }
    }
}

impl crate::session::state::SessionState {
    /// Scope native work by backend, conversation, turn and display part.
    pub(crate) fn backend_entry_key(&self, item_id: &str) -> BackendEntryKey {
        let backend = if self.codex_scope().enabled() {
            BackendKind::Codex
        } else if self.claude().enabled() {
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
            conversation: self.entry_conversation().clone(),
            turn: self
                .codex_item_turn()
                .clone()
                .filter(|_| self.codex_scope().enabled()),
            item,
            part: self.codex_item_part(),
        }
    }
}
