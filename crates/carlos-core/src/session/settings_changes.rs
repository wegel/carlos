//! Check a settings intent before native I/O and capture the defaults that its user requested.

use carlos_proto::commands::RpcError;
use carlos_proto::ids::SettingsRevision;
use carlos_proto::session::Backend;
use carlos_proto::settings::{SettingValue, SettingsChange};
use serde_json::{json, Value};

use super::runtime_settings::RuntimeDefaults;
use super::state::SessionState;
use crate::backend_commands::SettingsPatch;

/// One checked user intent, independent of later metadata or another client's editor.
pub(crate) struct SettingsIntent {
    pub defaults: RuntimeDefaults,
    pub native: Value,
}

impl SessionState {
    /// Validate the original settings/catalog revision before constructing a native request.
    pub(crate) fn prepare_settings_change(
        &self,
        expected: &SettingsRevision,
        change: SettingsChange,
    ) -> Result<SettingsIntent, RpcError> {
        self.check_settings_revision(expected)?;
        let mut defaults = self.settings_defaults();
        let mut native = json!({});
        for (name, value, destination) in [
            ("model", &change.model, &mut defaults.model),
            ("effort", &change.effort, &mut defaults.effort),
            ("summary", &change.summary, &mut defaults.summary),
        ] {
            if let Some(value) = value {
                self.check_setting(name, value)?;
                match value {
                    SettingValue::Value(value) => {
                        *destination = Some(value.clone());
                        native[name] = json!(value);
                    }
                    SettingValue::Inherit => {
                        *destination = None;
                        native[name] = Value::Null;
                    }
                }
            }
        }
        let clear_effort = self.check_model_choices(&change, &defaults)?;
        if clear_effort {
            defaults.effort = None;
            if self.events.backend == Backend::Claude {
                native["effort"] = Value::Null;
            }
        }
        if self.events.backend == Backend::Claude && defaults.effort.as_deref() == Some("auto") {
            defaults.effort = None;
            if native.get("effort").is_some() {
                native["effort"] = Value::Null;
            }
        }
        Ok(SettingsIntent { defaults, native })
    }

    fn check_settings_revision(&self, expected: &SettingsRevision) -> Result<(), RpcError> {
        if expected != &self.settings_revision() {
            return Err(invalid(
                "settings changed; refresh current settings before saving this draft",
            ));
        }
        if self.events.backend_closed {
            return Err(invalid(
                "native backend is disconnected; your choices have not been sent",
            ));
        }
        if self.runtime.native_settings_pending
            || self.codex_settings.pending.is_some()
            || self.codex_settings.deferred.is_some()
        {
            return Err(invalid(
                "another native settings request is still pending; nothing sent",
            ));
        }
        Ok(())
    }

    /// Capture explicit overrides for persistence; observed inherited Claude values stay inherited.
    pub(crate) fn settings_defaults(&self) -> RuntimeDefaults {
        let (model, effort, summary) = self.next_turn_runtime_settings();
        let (model, effort) = if self.events.backend == Backend::Claude {
            self.claude_prompt_choices(model, effort)
        } else {
            (model, effort)
        };
        RuntimeDefaults {
            model,
            effort,
            summary,
        }
    }

    /// Publish a held Claude request without replacing observed choices with guesses.
    pub(crate) fn set_native_settings_pending(&mut self, pending: bool) {
        self.runtime.native_settings_pending = pending;
    }

    /// Apply only fields the captured Claude helper confirmed, including explicit inheritance.
    pub(crate) fn accept_claude_settings(&mut self, patch: SettingsPatch) {
        if let Some(model) = patch.model {
            self.claude.inherits_model = model.is_none();
            self.runtime.current_model = model;
            self.runtime.pending_model = None;
        }
        if let Some(effort) = patch.effort {
            self.claude.inherits_effort = effort.is_none();
            self.runtime.current_effort = effort;
            self.runtime.pending_effort = None;
        }
        self.invalidate_claude_context();
    }

    fn check_setting(&self, name: &str, value: &SettingValue) -> Result<(), RpcError> {
        if name == "effort" && !self.runtime.supports_effort()
            || name == "summary" && !self.runtime.supports_summary()
        {
            return Err(invalid(
                "this backend does not advertise that settings field",
            ));
        }
        let SettingValue::Value(value) = value else {
            return if self.events.backend == Backend::Claude {
                Ok(())
            } else {
                Err(invalid(
                    "this backend cannot restore an inherited choice through its settings API",
                ))
            };
        };
        if value.trim().is_empty() {
            return Err(invalid("choose a nonempty advertised setting"));
        }
        if name == "summary" && !["auto", "concise", "detailed", "none"].contains(&value.as_str()) {
            return Err(invalid("choose a supported reasoning summary setting"));
        }
        Ok(())
    }

    fn check_model_choices(
        &self,
        change: &SettingsChange,
        defaults: &RuntimeDefaults,
    ) -> Result<bool, RpcError> {
        let models = self.runtime.available_models();
        if models.is_empty() || change.model.is_none() && change.effort.is_none() {
            return Ok(false);
        }
        let name = defaults
            .model
            .as_deref()
            .or(self.runtime.current_model().as_deref());
        let model = name.and_then(|name| models.iter().find(|model| model.model == name));
        if matches!(change.model, Some(SettingValue::Value(_))) && model.is_none() {
            return Err(invalid("choose a model advertised by this backend"));
        }
        if let Some(SettingValue::Value(effort)) = &change.effort {
            let model = model
                .ok_or_else(|| invalid("choose an advertised model before changing its effort"))?;
            if !model
                .supported_efforts
                .iter()
                .any(|option| option == effort)
            {
                return Err(invalid(
                    "choose an effort advertised for the selected model",
                ));
            }
        }
        Ok(matches!(change.model, Some(SettingValue::Value(_)))
            && change.effort.is_none()
            && model.is_some_and(|model| {
                model.supported_efforts.is_empty()
                    || self.events.backend == Backend::Claude
                        && model
                            .supported_efforts
                            .iter()
                            .all(|effort| effort == "auto")
            }))
    }
}

fn invalid(message: &str) -> RpcError {
    RpcError {
        code: -32012,
        message: message.into(),
    }
}
