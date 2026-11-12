//! Record native settings replies and discard only the overrides they settle.

use serde_json::Value;

use super::runtime_settings::{RuntimeDefaults, RuntimeSetting};
use super::state::SessionState;

impl SessionState {
    /// Record a native choice without trimming or changing other fields.
    pub(crate) fn record_runtime_choice(&mut self, setting: RuntimeSetting, value: Option<String>) {
        let current = match setting {
            RuntimeSetting::Model => &mut self.runtime.current_model,
            RuntimeSetting::Effort => &mut self.runtime.current_effort,
            RuntimeSetting::Summary => &mut self.runtime.current_summary,
        };
        *current = value;
    }

    /// Drop one pending override while retaining acknowledged choices.
    pub(crate) fn clear_pending_runtime_choice(&mut self, setting: RuntimeSetting) {
        match setting {
            RuntimeSetting::Model => self.runtime.pending_model = None,
            RuntimeSetting::Effort => self.runtime.pending_effort = None,
            RuntimeSetting::Summary => self.runtime.pending_summary = None,
        }
    }

    /// Restore native effort inheritance for both current and future turns.
    pub(crate) fn inherit_runtime_effort(&mut self) {
        self.runtime.current_effort = None;
        self.runtime.pending_effort = None;
    }

    /// Keep native launch overrides exactly as supplied, including empty strings.
    pub(crate) fn replace_pending_runtime_choices(&mut self, choices: RuntimeDefaults) {
        self.runtime.pending_model = choices.model;
        self.runtime.pending_effort = choices.effort;
        self.runtime.pending_summary = choices.summary;
    }

    /// Apply present native fields and retire only matching pending overrides.
    pub(crate) fn apply_future_runtime_settings(&mut self, settings: &Value) {
        for (key, current, pending) in [
            (
                "model",
                &mut self.runtime.current_model,
                &mut self.runtime.pending_model,
            ),
            (
                "effort",
                &mut self.runtime.current_effort,
                &mut self.runtime.pending_effort,
            ),
            (
                "summary",
                &mut self.runtime.current_summary,
                &mut self.runtime.pending_summary,
            ),
        ] {
            if let Some(value) = settings.get(key) {
                *current = value.as_str().map(str::to_string);
                if pending.as_deref() == value.as_str() {
                    *pending = None;
                }
            }
        }
    }

    /// Drop rejected overrides only if they still equal the submitted values.
    pub(crate) fn reject_runtime_settings(&mut self, desired: &Value) {
        for (key, pending) in [
            ("model", &mut self.runtime.pending_model),
            ("effort", &mut self.runtime.pending_effort),
            ("summary", &mut self.runtime.pending_summary),
        ] {
            if pending.as_deref() == desired[key].as_str() {
                *pending = None;
            }
        }
    }
}
