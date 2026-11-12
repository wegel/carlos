//! Model-settings overlay state: field cycling, value edits, and apply/persist logic.

use super::{default_effort_vec, effort_index, ModelSettingsField, RuntimeSettingsView};

use crate::session::runtime_settings::{
    normalize_non_empty, RuntimeDefaults, RuntimeSettingsState,
};

impl RuntimeSettingsView {
    /// List the visible settings in the same order for drawing and keyboard focus.
    pub(in crate::app) fn model_settings_fields(
        &self,
        runtime: &RuntimeSettingsState,
    ) -> Vec<ModelSettingsField> {
        let mut fields = vec![ModelSettingsField::Model];
        if runtime.supports_effort() && !self.model_settings_effort_options.is_empty() {
            fields.push(ModelSettingsField::Effort);
        }
        if runtime.supports_summary() {
            fields.push(ModelSettingsField::Summary);
        }
        if self.context.backend.is_some() {
            fields.push(ModelSettingsField::Context);
        }
        fields.extend([
            ModelSettingsField::DictationEndpoint,
            ModelSettingsField::ToolLabels,
            ModelSettingsField::DesktopTheme,
            ModelSettingsField::PhoneTheme,
            ModelSettingsField::ToolOutput,
        ]);
        fields
    }

    /// Collect edited choices without changing session facts or closing the overlay.
    pub(in crate::app) fn saved_choices(&self, runtime: &RuntimeSettingsState) -> RuntimeDefaults {
        let model = normalize_non_empty(self.model_settings_model_value(runtime).to_string());
        let effort = runtime
            .supports_effort()
            .then(|| normalize_non_empty(self.model_settings_effort_value().to_string()))
            .flatten();
        let summary = runtime
            .supports_summary()
            .then(|| normalize_non_empty(self.model_settings_summary_value(runtime).to_string()))
            .flatten();
        RuntimeDefaults {
            model,
            effort,
            summary,
        }
    }

    pub(in crate::app) fn model_settings_model_value<'a>(
        &'a self,
        runtime: &'a RuntimeSettingsState,
    ) -> &'a str {
        if let Some(model) = runtime
            .available_models()
            .get(self.model_settings_model_index)
        {
            return model.model.as_str();
        }
        self.model_settings_model_input.as_str()
    }

    pub(in crate::app) fn model_settings_effort_value(&self) -> &str {
        self.model_settings_effort_options
            .get(self.model_settings_effort_index)
            .map(String::as_str)
            .unwrap_or("medium")
    }

    pub(in crate::app) fn model_settings_summary_value(
        &self,
        runtime: &RuntimeSettingsState,
    ) -> &str {
        if !runtime.supports_summary() {
            return "";
        }
        self.model_settings_summary_options
            .get(self.model_settings_summary_index)
            .map(String::as_str)
            .unwrap_or("auto")
    }

    pub(super) fn refresh_model_settings_efforts(&mut self, runtime: &RuntimeSettingsState) {
        let requested = runtime
            .pending_effort()
            .as_deref()
            .or(runtime.current_effort().as_deref())
            .unwrap_or("medium")
            .to_string();

        let (options, default_effort) = if let Some(model) = runtime
            .available_models()
            .get(self.model_settings_model_index)
        {
            let options = if model.supported_efforts.is_empty() {
                default_effort_vec()
            } else {
                model.supported_efforts.clone()
            };
            (options, model.default_effort.clone())
        } else {
            (default_effort_vec(), None)
        };

        self.model_settings_effort_options = options;
        self.model_settings_effort_index = self
            .model_settings_effort_options
            .iter()
            .position(|e| e.eq_ignore_ascii_case(&requested))
            .or_else(|| {
                default_effort.as_deref().and_then(|d| {
                    self.model_settings_effort_options
                        .iter()
                        .position(|e| e.eq_ignore_ascii_case(d))
                })
            })
            .unwrap_or_else(|| {
                effort_index("medium")
                    .min(self.model_settings_effort_options.len().saturating_sub(1))
            });
    }
}
