//! Own native Codex flags, request serial and temporary item-key scope.

use super::scope::CodexScope;
use crate::session::state::SessionState;

/// Keep native scope and request counters private to the session.
#[derive(Debug, Default)]
pub(in crate::session) struct CodexMetadata {
    pub(in crate::session) scope: CodexScope,
    pub(in crate::session) serial: u64,
    name_revision: u64,
    item_turn: Option<String>,
    item_part: usize,
}

impl SessionState {
    /// Read the current native connection flags and epoch without a mutable fact reference.
    pub fn codex_scope(&self) -> CodexScope {
        self.codex_meta.scope
    }
    /// Enable native Codex handling without changing other backend flags.
    pub fn enable_codex(&mut self) {
        self.codex_meta.scope.enabled = true;
    }
    /// Fence native work after transport closure.
    pub fn close_codex(&mut self) {
        self.codex_meta.scope.closed = true;
    }
    /// Reopen native work only after its complete snapshot was installed.
    pub fn reopen_codex(&mut self) {
        self.codex_meta.scope.closed = false;
    }
    /// Invalidate old root operations with the original saturated epoch counter.
    pub(crate) fn advance_codex_epoch(&mut self) {
        self.codex_meta.scope.epoch = self.codex_meta.scope.epoch.saturating_add(1);
    }
    /// Borrow the temporary native turn used to construct an entry source key.
    pub fn codex_item_turn(&self) -> &Option<String> {
        &self.codex_meta.item_turn
    }
    /// Read the temporary display part used to construct an entry source key.
    pub fn codex_item_part(&self) -> usize {
        self.codex_meta.item_part
    }
    /// Enter or restore a native turn scope without cloning the previous scope.
    pub fn replace_codex_item_turn(&mut self, turn: Option<String>) -> Option<String> {
        std::mem::replace(&mut self.codex_meta.item_turn, turn)
    }
    /// Enter or restore a native display-part scope without duplicating its facts.
    pub fn replace_codex_item_part(&mut self, part: usize) -> usize {
        std::mem::replace(&mut self.codex_meta.item_part, part)
    }
    #[cfg(any(test, feature = "test-support"))]
    /// Enable or disable the native Codex routes in this fixture.
    pub fn set_codex_enabled_for_test(&mut self, enabled: bool) {
        self.codex_meta.scope.enabled = enabled;
    }
}

impl SessionState {
    /// Read the native title revision for a delayed metadata reply.
    pub(crate) fn codex_name_revision(&self) -> u64 {
        self.codex_meta.name_revision
    }

    /// Keep one safe title and invalidate older title reads.
    pub fn observe_name(&mut self, name: Option<&str>) {
        if self.codex_scope().enabled() {
            self.codex_meta.name_revision = self.codex_meta.name_revision.saturating_add(1);
        }
        self.set_thread_name(
            name.map(crate::session::tools::strip_terminal_controls)
                .map(|s| {
                    s.chars()
                        .map(|c| if c.is_control() { ' ' } else { c })
                        .collect::<String>()
                })
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty()),
        );
    }

    /// Accept a title only when the native record belongs to this thread.
    pub fn observe_thread_name(&mut self, thread: &serde_json::Value) {
        if thread.get("id").and_then(serde_json::Value::as_str) == Some(self.thread_id().as_str()) {
            if let Some(name) = thread.get("name") {
                self.observe_name(name.as_str());
            }
        }
    }
}
