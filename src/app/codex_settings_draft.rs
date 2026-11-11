//! Validate settings before changing runtime state and preserve rejected editor choices.

use serde_json::{json, Value};

use super::state::AppState;
use super::{persist_runtime_defaults, RuntimeDefaults};
use crate::protocol_params::parse_thread_runtime_settings;

const THREAD_SUMMARY: &str = "Use thread setting";

/// Apply Carlos defaults to new threads and only explicit launch choices to resumed threads.
pub(super) fn initialize(
    app: &mut AppState,
    response: &str,
    resumed: bool,
    summary_override: Option<String>,
) -> anyhow::Result<()> {
    let mut desired = if resumed { json!({}) } else { choices(app) };
    if let Some(summary) = summary_override {
        desired["summary"] = json!(summary);
    }
    let native = parse_thread_runtime_settings(response)?;
    app.set_runtime_settings(native.model, native.effort, native.summary);
    app.queue_runtime_settings(None, None, None);
    app.codex.settings.future = choices(app);
    desired
        .as_object_mut()
        .expect("settings object")
        .retain(|key, value| app.codex.settings.future.get(key) != Some(value));
    app.runtime.pending_model = desired["model"].as_str().map(str::to_string);
    app.runtime.pending_effort = desired["effort"].as_str().map(str::to_string);
    app.runtime.pending_summary = desired["summary"].as_str().map(str::to_string);
    app.codex.settings.deferred =
        (!desired.as_object().expect("settings object").is_empty()).then_some(desired);
    Ok(())
}

/// Keep ordinary prompts free of cached overrides unless native definitively lacks settings updates.
pub(super) fn choices(app: &AppState) -> Value {
    let (model, effort, summary) = app.next_turn_runtime_settings();
    let mut desired = json!({});
    for (key, value) in [("model", model), ("effort", effort), ("summary", summary)] {
        if let Some(value) = value {
            desired[key] = json!(value);
        }
    }
    if !supports_effort(app, desired["model"].as_str().unwrap_or("")) {
        desired
            .as_object_mut()
            .expect("settings object")
            .remove("effort");
    }
    desired
}

/// Reject an editor choice before it can become a turn override or a saved default.
pub(super) fn validate_editor(app: &mut AppState) -> bool {
    let mut desired = json!({"model":app.model_settings_model_value(),
        "effort":app.model_settings_effort_value(),"summary":app.model_settings_summary_value()});
    if !supports_effort(app, app.model_settings_model_value()) {
        desired
            .as_object_mut()
            .expect("settings object")
            .remove("effort");
    }
    if desired["summary"] == THREAD_SUMMARY {
        desired
            .as_object_mut()
            .expect("settings object")
            .remove("summary");
    }
    match super::codex_settings::validate(app, &desired) {
        Ok(()) => {
            app.codex.settings.editor_error = None;
            true
        }
        Err(error) => {
            app.codex.settings.editor_error = Some(error);
            false
        }
    }
}

/// Hide effort controls when the selected native model advertises no effort choices.
pub(super) fn refresh_editor(app: &mut AppState) {
    if !supports_effort(app, app.model_settings_model_value()) {
        app.runtime.model_settings_effort_options.clear();
    }
    if app.runtime.current_summary.is_none()
        && app.runtime.pending_summary.is_none()
        && !app
            .runtime
            .model_settings_summary_options
            .iter()
            .any(|value| value == THREAD_SUMMARY)
    {
        app.runtime
            .model_settings_summary_options
            .insert(0, THREAD_SUMMARY.into());
        app.runtime.model_settings_summary_index = 0;
    }
}

/// Show an inherited unknown summary without turning its displayed fallback into a choice.
pub(super) fn open_editor(app: &mut AppState) {
    refresh_editor(app);
    if app.runtime.current_summary.is_none() && app.runtime.pending_summary.is_none() {
        app.runtime.model_settings_summary_index = app
            .runtime
            .model_settings_summary_options
            .iter()
            .position(|value| value == THREAD_SUMMARY)
            .unwrap_or(0);
    }
}

/// Keep an unadvertised effort out of the saved choices when changing model families.
pub(super) fn normalize_saved(app: &mut AppState, defaults: &mut RuntimeDefaults) {
    if defaults.summary.as_deref() == Some(THREAD_SUMMARY) {
        defaults.summary = None;
        app.runtime.pending_summary = None;
    }
    if !supports_effort(app, defaults.model.as_deref().unwrap_or("")) {
        defaults.effort = None;
        app.runtime.pending_effort = None;
    }
}

fn supports_effort(app: &AppState, model: &str) -> bool {
    app.runtime
        .available_models
        .iter()
        .find(|entry| entry.model == model)
        .is_none_or(|entry| !entry.supported_efforts.is_empty())
}

/// Keep the rejected values while removing only their pending turn overrides.
pub(super) fn reject(app: &mut AppState, desired: &Value, error: Value) {
    let message = super::claude_question_state::safe_text(
        error["message"]
            .as_str()
            .unwrap_or("Codex rejected these choices"),
    );
    if app.codex.settings.deferred.is_none() {
        app.codex.settings.rejected = Some(desired.clone());
        app.codex.settings.editor_error = Some(message);
    }
    for (key, pending) in [
        ("model", &mut app.runtime.pending_model),
        ("effort", &mut app.runtime.pending_effort),
        ("summary", &mut app.runtime.pending_summary),
    ] {
        if pending.as_deref() == desired[key].as_str() {
            *pending = None;
        }
    }
}

/// Reopen attempted values without changing last-valid runtime settings.
pub(super) fn restore_editor(app: &mut AppState) {
    let Some(desired) = app.codex.settings.rejected.clone() else {
        return;
    };
    if let Some(model) = desired["model"].as_str() {
        app.runtime.model_settings_model_index = app
            .runtime
            .available_models
            .iter()
            .position(|entry| entry.model == model)
            .unwrap_or(usize::MAX);
        app.runtime.model_settings_model_input = model.into();
        if app.runtime.model_settings_model_index != usize::MAX {
            app.runtime.model_settings_cycle_model(0);
        }
    }
    if let Some(effort) = desired["effort"].as_str() {
        let options = &mut app.runtime.model_settings_effort_options;
        let index = options
            .iter()
            .position(|entry| entry == effort)
            .unwrap_or_else(|| {
                options.push(effort.into());
                options.len() - 1
            });
        app.runtime.model_settings_effort_index = index;
    }
    if let Some(summary) = desired["summary"].as_str() {
        if let Some(index) = app
            .runtime
            .model_settings_summary_options
            .iter()
            .position(|entry| entry == summary)
        {
            app.runtime.model_settings_summary_index = index;
        }
    }
}

/// Save Carlos defaults only after native acceptance or a precise compatibility fallback.
pub(super) fn persist(app: &mut AppState, desired: &Value) {
    let defaults = RuntimeDefaults {
        model: desired["model"].as_str().map(str::to_string),
        effort: desired["effort"].as_str().map(str::to_string),
        summary: desired["summary"].as_str().map(str::to_string),
    };
    if let Err(error) = persist_runtime_defaults(&defaults) {
        app.append_message(
            super::Role::System,
            format!("Codex accepted the choices, but Carlos could not save its defaults: {error}"),
        );
    }
}
