//! Apply captured Carlos choices once after initial native runtime and catalog facts arrive.

use serde_json::{json, Value};

use crate::protocol_params::ThreadRuntimeSettings;
use crate::session::runtime_settings::RuntimeDefaults;
use crate::session::runtime_startup::{codex_runtime_defaults, resolve_backend_runtime_settings};
use crate::session::runtime_startup_catalog::complete_codex_model_catalog;
use crate::session::state::SessionState;

#[derive(Debug)]
pub(in crate::session) struct StartupChoices {
    resumed: bool,
    summary_override: Option<String>,
}

impl SessionState {
    /// Capture deliberate launch intent without treating fallback choices as native facts.
    pub(crate) fn prepare_codex_startup(
        &mut self,
        resumed: bool,
        summary_override: Option<String>,
    ) {
        self.codex_connection.startup = Some(StartupChoices {
            resumed,
            summary_override,
        });
    }

    pub(in crate::session) fn install_codex_startup(&mut self) {
        let Some(startup) = self.codex_connection.startup.take() else {
            return;
        };
        if self.runtime_settings().available_models().is_empty() {
            self.set_available_models(complete_codex_model_catalog(Vec::new()));
        }
        let mut desired = if startup.resumed {
            json!({})
        } else {
            self.new_codex_choices()
        };
        if let Some(summary) = startup.summary_override {
            desired["summary"] = json!(summary);
        }
        desired
            .as_object_mut()
            .expect("startup choices")
            .retain(|key, value| self.codex_settings().future().get(key) != Some(value));
        self.replace_pending_runtime_choices(RuntimeDefaults {
            model: desired["model"].as_str().map(str::to_string),
            effort: desired["effort"].as_str().map(str::to_string),
            summary: desired["summary"].as_str().map(str::to_string),
        });
        self.codex_connection.initial_choices =
            (!desired.as_object().expect("startup choices").is_empty()).then_some(desired);
    }

    fn new_codex_choices(&self) -> Value {
        let preferences = self.runtime_settings().startup_preferences();
        let native = ThreadRuntimeSettings {
            model: preferences
                .model
                .clone()
                .or_else(|| self.runtime_settings().current_model().clone()),
            effort: preferences
                .effort
                .clone()
                .or_else(|| self.runtime_settings().current_effort().clone()),
            summary: self.runtime_settings().current_summary().clone(),
        };
        let selected = resolve_backend_runtime_settings(
            native,
            preferences,
            &codex_runtime_defaults(None),
            self.runtime_settings().available_models(),
            true,
        );
        let mut desired = json!({});
        for (key, value) in [
            ("model", selected.model),
            ("effort", selected.effort),
            ("summary", selected.summary),
        ] {
            if let Some(value) = value {
                desired[key] = json!(value);
            }
        }
        if self
            .runtime_settings()
            .available_models()
            .iter()
            .find(|model| Some(model.model.as_str()) == desired["model"].as_str())
            .is_some_and(|model| model.supported_efforts.is_empty())
        {
            desired
                .as_object_mut()
                .expect("startup choices")
                .remove("effort");
        }
        desired
    }
}

#[cfg(test)]
#[path = "startup_tests.rs"]
mod tests;
