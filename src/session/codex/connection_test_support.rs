//! Set exact native connection fixtures without exporting mutable facts.

use crate::session::state::SessionState;
use std::path::PathBuf;

impl SessionState {
    /// Select the fixture transport without exposing its connection fields.
    pub(crate) fn set_codex_socket_for_test(&mut self, socket: Option<PathBuf>) {
        self.codex_connection.socket = socket;
    }
    /// Select the generation accepted by a fixed native fixture.
    pub(crate) fn set_codex_generation_for_test(&mut self, generation: u64) {
        self.codex_connection.generation = generation;
    }
    /// Place the fixed fixture before or after native snapshot adoption.
    pub(crate) fn set_codex_rebuilding_for_test(&mut self, rebuilding: bool) {
        self.codex_connection.rebuilding = rebuilding;
    }
    /// Enable or retire the fixture metadata read at its original timer point.
    pub(crate) fn set_codex_runtime_refresh_for_test(&mut self, refresh: bool) {
        self.codex_connection.runtime_refresh = refresh;
    }
}
