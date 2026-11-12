//! Own native Codex flags, request serial and temporary item-key scope.

use super::scope::CodexScope;
use crate::session::state::SessionState;

#[derive(Debug, Default)]
pub(in crate::session) struct CodexMetadata {
    pub(in crate::session) scope: CodexScope,
    pub(in crate::session) serial: u64,
    item_turn: Option<String>,
    item_part: usize,
}

impl SessionState {
    /// Read the current native connection flags and epoch without a mutable fact reference.
    pub(crate) fn codex_scope(&self) -> CodexScope {
        self.codex_meta.scope
    }
    /// Enable native Codex handling without changing other backend flags.
    pub(crate) fn enable_codex(&mut self) {
        self.codex_meta.scope.enabled = true;
    }
    /// Fence native work after transport closure.
    pub(crate) fn close_codex(&mut self) {
        self.codex_meta.scope.closed = true;
    }
    /// Reopen native work only after its complete snapshot was installed.
    pub(crate) fn reopen_codex(&mut self) {
        self.codex_meta.scope.closed = false;
    }
    /// Invalidate old root operations with the original saturated epoch counter.
    pub(crate) fn advance_codex_epoch(&mut self) {
        self.codex_meta.scope.epoch = self.codex_meta.scope.epoch.saturating_add(1);
    }
    /// Borrow the temporary native turn used to construct an entry source key.
    pub(crate) fn codex_item_turn(&self) -> &Option<String> {
        &self.codex_meta.item_turn
    }
    /// Read the temporary display part used to construct an entry source key.
    pub(crate) fn codex_item_part(&self) -> usize {
        self.codex_meta.item_part
    }
    /// Enter or restore a native turn scope without cloning the previous scope.
    pub(crate) fn replace_codex_item_turn(&mut self, turn: Option<String>) -> Option<String> {
        std::mem::replace(&mut self.codex_meta.item_turn, turn)
    }
    /// Enter or restore a native display-part scope without duplicating its facts.
    pub(crate) fn replace_codex_item_part(&mut self, part: usize) -> usize {
        std::mem::replace(&mut self.codex_meta.item_part, part)
    }
    #[cfg(test)]
    pub(crate) fn set_codex_enabled_for_test(&mut self, enabled: bool) {
        self.codex_meta.scope.enabled = enabled;
    }
    #[cfg(test)]
    pub(crate) fn set_codex_closed_for_test(&mut self, closed: bool) {
        self.codex_meta.scope.closed = closed;
    }
}
