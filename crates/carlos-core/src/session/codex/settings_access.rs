//! Expose accepted settings and named startup/reconnect commands.

use serde_json::Value;

use super::settings_state::SettingsState;
use crate::session::state::SessionState;

impl SessionState {
    pub(crate) fn retire_owned_settings(
        &mut self,
        operation: &carlos_proto::submissions::ClientOperation,
    ) {
        if self
            .codex_settings
            .pending
            .as_ref()
            .is_some_and(|change| change.request.operation.as_ref() == Some(operation))
        {
            self.codex_settings.pending = None;
        }
    }
    /// Borrow native settings facts without exposing pending writes for mutation.
    pub fn codex_settings(&self) -> &SettingsState {
        &self.codex_settings
    }
    /// Set the acknowledged future choices from a validated startup response.
    pub fn set_codex_future_settings(&mut self, future: Value) {
        self.codex_settings.future = future;
    }
    /// Replace accepted deferred work without changing its requested side effects.
    pub fn defer_codex_settings(&mut self, desired: Option<Value>) {
        self.codex_settings.deferred = desired;
    }
    /// Move initial accepted choices to the reconnect reader once.
    pub(crate) fn take_deferred_codex_settings(&mut self) -> Option<Value> {
        self.codex_settings.deferred.take()
    }
    /// Reset native thread settings while preserving the known legacy API fallback.
    pub(crate) fn reset_codex_settings(&mut self) {
        let legacy_overrides = self.codex_settings.legacy_overrides;
        self.codex_settings = Default::default();
        self.codex_settings.legacy_overrides = legacy_overrides;
    }
    /// Install authoritative choices after a validated reconnect snapshot.
    pub(crate) fn install_codex_settings(&mut self, future: Value) {
        self.codex_settings.future = future;
        self.codex_settings.legacy_overrides = false;
        self.codex_settings.deferred = None;
        self.codex_settings.pending = None;
        self.codex_settings.active_before = None;
    }
}

impl SessionState {
    /// Read future choices using the native catalog's existing effort-capability rule.
    pub fn codex_runtime_choices(&self) -> serde_json::Value {
        let (model, effort, summary) = self.next_turn_runtime_settings();
        let mut desired = serde_json::json!({});
        for (key, value) in [("model", model), ("effort", effort), ("summary", summary)] {
            if let Some(value) = value {
                desired[key] = serde_json::json!(value);
            }
        }
        if self
            .runtime_settings()
            .available_models()
            .iter()
            .find(|entry| entry.model == desired["model"].as_str().unwrap_or(""))
            .is_some_and(|entry| entry.supported_efforts.is_empty())
        {
            desired
                .as_object_mut()
                .expect("settings object")
                .remove("effort");
        }
        desired
    }
}
