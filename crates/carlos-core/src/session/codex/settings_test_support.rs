//! Seed native settings fixtures without exposing mutable settings records.

use crate::session::state::SessionState;

impl SessionState {
    pub fn clear_codex_settings_pending_for_test(&mut self) {
        self.codex_settings.pending = None;
    }
    pub fn set_codex_settings_legacy_for_test(&mut self, legacy: bool) {
        self.codex_settings.legacy_overrides = legacy;
    }
}
