//! Set exact native connection fixtures without exporting mutable facts.

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::codex_snapshot::CodexSnapshot;
use crate::session::state::SessionState;

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

/// Build a complete same-thread native snapshot without a terminal fixture.
pub(super) fn snapshot() -> CodexSnapshot {
    CodexSnapshot {
        thread: json!({"id":"root","status":{"type":"idle"},"turns":[{"id":"turn","status":"completed","items":[{"id":"item","type":"agentMessage","text":"original"}]}]}),
        runtime: json!({"result":{"thread":{"id":"root"},"model":"native-model"}}).to_string(),
        catalog: json!({"data":[{"model":"native-model","displayName":"Native"}]}),
        queue: None,
        goal: None,
        panel: Value::Null,
    }
}
