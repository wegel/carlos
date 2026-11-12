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
