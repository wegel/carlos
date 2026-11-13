//! Validate settings before changing runtime state and preserve rejected editor choices.

use carlos_core::protocol_params::parse_thread_runtime_settings;
use carlos_core::session::runtime_settings::RuntimeDefaults;
use carlos_core::session::runtime_settings::RuntimeSetting;
use serde_json::{json, Value};

use super::persist_runtime_defaults;
use super::state::AppState;

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
    let future = choices(app);
    app.session.set_codex_future_settings(future);
    desired
        .as_object_mut()
        .expect("settings object")
        .retain(|key, value| app.session.codex_settings().future().get(key) != Some(value));
    app.session
        .replace_pending_runtime_choices(RuntimeDefaults {
            model: desired["model"].as_str().map(str::to_string),
            effort: desired["effort"].as_str().map(str::to_string),
            summary: desired["summary"].as_str().map(str::to_string),
        });
    app.session.defer_codex_settings(
        (!desired.as_object().expect("settings object").is_empty()).then_some(desired),
    );
    Ok(())
}

/// Keep ordinary prompts free of cached overrides unless native definitively lacks settings updates.
pub(super) fn choices(app: &AppState) -> Value {
    app.session.codex_runtime_choices()
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
            app.view.codex_settings.editor_error = None;
            true
        }
        Err(error) => {
            app.view.codex_settings.editor_error = Some(error);
            false
        }
    }
}

/// Hide effort controls when the selected native model advertises no effort choices.
pub(super) fn refresh_editor(app: &mut AppState) {
    if !supports_effort(app, app.model_settings_model_value()) {
        app.view
            .runtime_settings
            .model_settings_effort_options
            .clear();
    }
    if app.session.runtime_settings().current_summary().is_none()
        && app.session.runtime_settings().pending_summary().is_none()
        && !app
            .view
            .runtime_settings
            .model_settings_summary_options
            .iter()
            .any(|value| value == THREAD_SUMMARY)
    {
        app.view
            .runtime_settings
            .model_settings_summary_options
            .insert(0, THREAD_SUMMARY.into());
        app.view.runtime_settings.model_settings_summary_index = 0;
    }
}

/// Show an inherited unknown summary without turning its displayed fallback into a choice.
pub(super) fn open_editor(app: &mut AppState) {
    refresh_editor(app);
    if app.session.runtime_settings().current_summary().is_none()
        && app.session.runtime_settings().pending_summary().is_none()
    {
        app.view.runtime_settings.model_settings_summary_index = app
            .view
            .runtime_settings
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
        app.session
            .clear_pending_runtime_choice(RuntimeSetting::Summary);
    }
    if !supports_effort(app, defaults.model.as_deref().unwrap_or("")) {
        defaults.effort = None;
        app.session
            .clear_pending_runtime_choice(RuntimeSetting::Effort);
    }
}

fn supports_effort(app: &AppState, model: &str) -> bool {
    app.session
        .runtime_settings()
        .available_models()
        .iter()
        .find(|entry| entry.model == model)
        .is_none_or(|entry| !entry.supported_efforts.is_empty())
}

/// Reopen attempted values without changing last-valid runtime settings.
pub(super) fn restore_editor(app: &mut AppState) {
    let Some(desired) = app.view.codex_settings.rejected.clone() else {
        return;
    };
    if let Some(model) = desired["model"].as_str() {
        app.view.runtime_settings.model_settings_model_index = app
            .session
            .runtime_settings()
            .available_models()
            .iter()
            .position(|entry| entry.model == model)
            .unwrap_or(usize::MAX);
        app.view.runtime_settings.model_settings_model_input = model.into();
        if app.view.runtime_settings.model_settings_model_index != usize::MAX {
            app.model_settings_cycle_model(0);
        }
    }
    if let Some(effort) = desired["effort"].as_str() {
        let options = &mut app.view.runtime_settings.model_settings_effort_options;
        let index = options
            .iter()
            .position(|entry| entry == effort)
            .unwrap_or_else(|| {
                options.push(effort.into());
                options.len() - 1
            });
        app.view.runtime_settings.model_settings_effort_index = index;
    }
    if let Some(summary) = desired["summary"].as_str() {
        if let Some(index) = app
            .view
            .runtime_settings
            .model_settings_summary_options
            .iter()
            .position(|entry| entry == summary)
        {
            app.view.runtime_settings.model_settings_summary_index = index;
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
            carlos_core::session::models::Role::System,
            format!("Codex accepted the choices, but Carlos could not save its defaults: {error}"),
        );
    }
}
