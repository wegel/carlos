//! Resolves Carlos's saved choices against native Claude models before sending a prompt.

use serde_json::Value;

use crate::protocol_params::ModelInfo;

use super::state::AppState;
use super::RuntimeDefaults;

/// Validate saved Carlos choices against native Claude model metadata.
pub(super) fn apply_capabilities(app: &mut AppState, capabilities: &Value) {
    if app.claude.phase == crate::app::claude_state::ClaudePhase::Closed {
        return;
    }
    let requested = app
        .runtime
        .pending_model
        .clone()
        .or_else(|| app.runtime.current_model.clone());
    let observed = app.claude.observed_model.as_deref();
    let explicit_model = !app.claude.selection.inherits_model
        || app.runtime.pending_model.is_some()
        || (app.claude.capabilities.is_none()
            && requested.is_some()
            && requested.as_deref() != observed);
    let supported = requested
        .as_deref()
        .is_some_and(|model| find_model(capabilities, model).is_some() || Some(model) == observed);
    let selected = if supported {
        requested.clone()
    } else {
        observed.map(|model| native_alias(capabilities, model))
    };
    if requested.is_some() && !supported {
        app.runtime.pending_model = None;
        app.runtime.pending_effort = None;
        app.runtime.current_effort = None;
        app.claude.selection.inherits_effort = true;
    }
    app.claude.selection.inherits_model = !supported || !explicit_model;
    app.runtime.current_model = selected;
    validate_effort(app, capabilities);
    app.claude.capabilities = Some(capabilities.clone());
    refresh_models(app);
    if !app.claude.awaiting_reset_init {
        app.claude.phase = crate::app::claude_state::ClaudePhase::Ready;
    }
    app.set_status("Claude ready");
    if let Some(text) = app.claude.history.edited_prompt.take() {
        super::claude_edit::retire_queue(app);
        super::claude_edit::queued(app, &text);
        app.clear_rewind_mode_state();
        if app.input_text() == text {
            app.clear_input();
        }
        app.queue_turn_input_with_history(text, true);
    }
}

/// Record a native model without overwriting an explicit Carlos choice.
pub(super) fn observe_model(app: &mut AppState, model: &str) {
    if app
        .claude
        .observed_model
        .as_deref()
        .is_some_and(|previous| previous != model)
    {
        super::claude_context::invalidate(app);
    }
    app.claude.observed_model = Some(model.to_string());
    if app.runtime.current_model.is_none() && app.runtime.pending_model.is_none() {
        app.runtime.current_model = Some(
            app.claude
                .capabilities
                .as_ref()
                .map(|capabilities| native_alias(capabilities, model))
                .unwrap_or_else(|| model.to_string()),
        );
        refresh_models(app);
    }
}

fn validate_effort(app: &mut AppState, capabilities: &Value) {
    let Some(model) = app
        .runtime
        .current_model
        .as_deref()
        .and_then(|model| find_model(capabilities, model))
    else {
        return;
    };
    let effort = app
        .runtime
        .pending_effort
        .as_deref()
        .or(app.runtime.current_effort.as_deref());
    let Some(effort) = effort.filter(|effort| *effort != "auto") else {
        return;
    };
    let invalid = model["supportsEffort"] == false
        || model["supportedEffortLevels"]
            .as_array()
            .is_some_and(|levels| !levels.iter().any(|level| level.as_str() == Some(effort)));
    if invalid {
        app.runtime.current_effort = None;
        app.runtime.pending_effort = None;
        app.claude.selection.inherits_effort = true;
    }
}

/// Omit inherited choices so native CLI settings remain in charge.
pub(super) fn prompt_choices(
    app: &AppState,
    model: Option<String>,
    effort: Option<String>,
) -> (Option<String>, Option<String>) {
    (
        model.filter(|_| !app.claude.selection.inherits_model),
        effort.filter(|_| !app.claude.selection.inherits_effort),
    )
}

/// Remember the initial values when the user opens Claude settings.
pub(super) fn begin_settings_edit(app: &mut AppState) {
    if app.claude.selection.editing.is_none() {
        let choices = (
            app.runtime.model_settings_model_value().to_string(),
            app.runtime.model_settings_effort_value().to_string(),
        );
        app.claude.selection.editing = Some(choices);
    }
}

/// Keep appearance-only saves from pinning inherited model or effort values.
pub(super) fn prepare_saved_choices(app: &mut AppState, defaults: &mut RuntimeDefaults) {
    let Some((model, effort)) = app.claude.selection.editing.as_ref() else {
        return;
    };
    let model_changed = defaults
        .model
        .as_deref()
        .is_some_and(|choice| choice != model);
    let effort_changed = defaults
        .effort
        .as_deref()
        .is_some_and(|choice| choice != effort);
    if app.claude.selection.inherits_model && !model_changed {
        defaults.model = None;
        app.runtime.pending_model = None;
    } else {
        app.claude.selection.inherits_model = false;
    }
    if (app.claude.selection.inherits_effort && !effort_changed)
        || defaults
            .effort
            .as_deref()
            .is_none_or(|effort| effort == "auto")
    {
        defaults.effort = None;
        app.runtime.current_effort = None;
        app.runtime.pending_effort = None;
        app.claude.selection.inherits_effort = true;
    } else {
        app.claude.selection.inherits_effort = false;
    }
}

fn refresh_models(app: &mut AppState) {
    let Some(capabilities) = app.claude.capabilities.as_ref() else {
        return;
    };
    let selected = app.runtime.current_model.as_deref();
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
    app.set_runtime_capabilities(
        models.iter().any(|model| {
            model
                .supported_efforts
                .iter()
                .any(|effort| effort != "auto")
        }),
        false,
    );
    app.set_available_models(models);
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
