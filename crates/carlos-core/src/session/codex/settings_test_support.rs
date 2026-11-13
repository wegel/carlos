//! Seed native settings fixtures without exposing mutable settings records.

use crate::session::state::SessionState;

impl SessionState {
    /// Clear the fixture native settings request.
    pub fn clear_codex_settings_pending_for_test(&mut self) {
        self.codex_settings.pending = None;
    }
    /// Select the fixture fallback for servers without native settings controls.
    pub fn set_codex_settings_legacy_for_test(&mut self, legacy: bool) {
        self.codex_settings.legacy_overrides = legacy;
    }
}
