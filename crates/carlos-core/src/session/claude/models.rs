//! Resolve saved model choices against native Claude metadata.

use serde_json::Value;

use super::state::ClaudePhase;
use crate::session::model_info::ModelInfo;
use crate::session::runtime_settings::RuntimeSetting;
use crate::session::state::SessionState;

impl SessionState {
    /// Validate saved choices before accepting native model metadata.
    pub(crate) fn apply_claude_capabilities(&mut self, capabilities: &Value) -> bool {
        if self.claude.phase == ClaudePhase::Closed {
            return false;
        }
        let requested = self
            .runtime_settings()
            .pending_model()
            .clone()
            .or_else(|| self.runtime_settings().current_model().clone());
        let observed = self.claude.observed_model.as_deref();
        let explicit_model = !self.claude.inherits_model
            || self.runtime_settings().pending_model().is_some()
            || (self.claude.capabilities.is_none()
                && requested.is_some()
                && requested.as_deref() != observed);
        let supported = requested.as_deref().is_some_and(|model| {
            find_model(capabilities, model).is_some() || Some(model) == observed
        });
        let selected = if supported {
            requested.clone()
        } else {
            observed.map(|model| native_alias(capabilities, model))
        };
        if requested.is_some() && !supported {
            self.clear_pending_runtime_choice(RuntimeSetting::Model);
            self.inherit_runtime_effort();
            self.claude.inherits_effort = true;
        }
        self.claude.inherits_model = !supported || !explicit_model;
        self.record_runtime_choice(RuntimeSetting::Model, selected);
        self.validate_effort(capabilities);
        self.claude.capabilities = Some(capabilities.clone());
        self.refresh_claude_models();
        if !self.claude.awaiting_reset_init {
            self.claude.phase = ClaudePhase::Ready;
        }
        self.set_status("Claude ready");
        true
    }

    /// Record a native model without overwriting an explicit Carlos choice.
    pub fn observe_claude_model(&mut self, model: &str) {
        if self
            .claude
            .observed_model
            .as_deref()
            .is_some_and(|previous| previous != model)
        {
            self.invalidate_claude_context();
        }
        self.claude.observed_model = Some(model.to_string());
        if self.runtime_settings().current_model().is_none()
            && self.runtime_settings().pending_model().is_none()
        {
            self.record_runtime_choice(
                RuntimeSetting::Model,
                Some(
                    self.claude
                        .capabilities
                        .as_ref()
                        .map(|capabilities| native_alias(capabilities, model))
                        .unwrap_or_else(|| model.to_string()),
                ),
            );
            self.refresh_claude_models();
        }
    }

    fn validate_effort(&mut self, capabilities: &Value) {
        let Some(model) = self
            .runtime_settings()
            .current_model()
            .as_deref()
            .and_then(|model| find_model(capabilities, model))
        else {
            return;
        };
        let effort = self
            .runtime_settings()
            .pending_effort()
            .as_deref()
            .or(self.runtime_settings().current_effort().as_deref());
        let Some(effort) = effort.filter(|effort| *effort != "auto") else {
            return;
        };
        let invalid = model["supportsEffort"] == false
            || model["supportedEffortLevels"]
                .as_array()
                .is_some_and(|levels| !levels.iter().any(|level| level.as_str() == Some(effort)));
        if invalid {
            self.inherit_runtime_effort();
            self.claude.inherits_effort = true;
        }
    }

    fn refresh_claude_models(&mut self) {
        let Some(capabilities) = self.claude.capabilities.as_ref() else {
            return;
        };
        let selected = self.runtime_settings().current_model().as_deref();
        let mut models: Vec<ModelInfo> = capabilities["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|model| {
                let value = model["value"].as_str()?;
                let identifier = selected
                    .filter(|selected| model["resolvedModel"].as_str() == Some(*selected))
                    .unwrap_or(value);
                Some(ModelInfo {
                    model: identifier.to_string(),
                    display_name: model["displayName"].as_str().unwrap_or(value).to_string(),
                    supported_efforts: native_efforts(model),
                    default_effort: None,
                    is_default: false,
                })
            })
            .collect();
        if let Some(model) =
            selected.filter(|selected| !models.iter().any(|model| model.model == *selected))
        {
            models.insert(
                0,
                ModelInfo {
                    model: model.to_string(),
                    display_name: model.to_string(),
                    supported_efforts: vec!["auto".to_string()],
                    default_effort: None,
                    is_default: false,
                },
            );
        }
        self.set_runtime_capabilities(
            models.iter().any(|model| {
                model
                    .supported_efforts
                    .iter()
                    .any(|effort| effort != "auto")
            }),
            false,
        );
        self.set_available_models(models);
    }
}

fn native_efforts(model: &Value) -> Vec<String> {
    model["supportedEffortLevels"]
        .as_array()
        .map(|levels| {
            levels
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .filter(|levels: &Vec<String>| model["supportsEffort"] != false && !levels.is_empty())
        .unwrap_or_else(|| vec!["auto".to_string()])
}

fn find_model<'a>(capabilities: &'a Value, name: &str) -> Option<&'a Value> {
    capabilities["models"]
        .as_array()?
        .iter()
        .find(|model| model["value"] == name || model["resolvedModel"] == name)
}

fn native_alias(capabilities: &Value, observed: &str) -> String {
    find_model(capabilities, observed)
        .and_then(|model| model["value"].as_str())
        .unwrap_or(observed)
        .to_string()
}
