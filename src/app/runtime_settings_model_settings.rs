//! Model-settings overlay state: field cycling, value edits, and apply/persist logic.

use super::{
    default_effort_vec, effort_index, normalize_non_empty, ModelSettingsField, RuntimeDefaults,
    RuntimeSettingsState,
};

impl RuntimeSettingsState {
    /// List the visible settings in the same order for drawing and keyboard focus.
    pub(in crate::app) fn model_settings_fields(&self) -> Vec<ModelSettingsField> {
        let mut fields = vec![ModelSettingsField::Model];
        if self.supports_effort {
            fields.push(ModelSettingsField::Effort);
        }
        if self.supports_summary {
            fields.push(ModelSettingsField::Summary);
        }
        fields.extend([
            ModelSettingsField::DictationEndpoint,
            ModelSettingsField::ToolOutput,
        ]);
        fields
    }

    pub(in crate::app) fn apply_model_settings(&mut self) -> RuntimeDefaults {
        let model = normalize_non_empty(self.model_settings_model_value().to_string());
        let effort = self
            .supports_effort
            .then(|| normalize_non_empty(self.model_settings_effort_value().to_string()))
            .flatten();
        let summary = self
            .supports_summary
            .then(|| normalize_non_empty(self.model_settings_summary_value().to_string()))
            .flatten();
        let defaults = RuntimeDefaults {
            model: model.clone(),
            effort: effort.clone(),
            summary: summary.clone(),
        };
        self.queue_runtime_settings(model, effort, summary);
        self.show_model_settings = false;
        defaults
    }

    pub(in crate::app) fn model_settings_model_value(&self) -> &str {
        if let Some(model) = self.available_models.get(self.model_settings_model_index) {
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

    pub(in crate::app) fn model_settings_summary_value(&self) -> &str {
        if !self.supports_summary {
            return "";
        }
        self.model_settings_summary_options
            .get(self.model_settings_summary_index)
            .map(String::as_str)
            .unwrap_or("auto")
    }

    #[cfg(test)]
    pub(in crate::app) fn apply_default_reasoning_summary(&mut self, summary: Option<String>) {
        if self.current_summary.is_none() && self.pending_summary.is_none() {
            self.pending_summary = summary.and_then(normalize_non_empty);
        }
    }

    pub(super) fn refresh_model_settings_efforts(&mut self) {
        let requested = self
            .pending_effort
            .as_deref()
            .or(self.current_effort.as_deref())
            .unwrap_or("medium")
            .to_string();

        let (options, default_effort) =
            if let Some(model) = self.available_models.get(self.model_settings_model_index) {
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
