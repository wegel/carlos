//! Bridge Claude metadata to local settings and edited-prompt widgets.

use serde_json::Value;

use super::state::AppState;
use super::RuntimeDefaults;

/// Validate native metadata before touching the saved edited-prompt queue.
pub(super) fn apply_capabilities(app: &mut AppState, capabilities: &Value) {
    if !app.session.apply_claude_capabilities(capabilities) {
        return;
    }
    if let Some(text) = super::claude_session_history::take_edit_prompt(app) {
        super::claude_edit::retire_queue(app);
        super::claude_edit::queued(app, &text);
        app.clear_rewind_mode_state();
        if app.input_text() == text {
            app.clear_input();
        }
        app.queue_turn_input_with_history(text, true);
    }
}

/// Fence context reads before recording a changed native model.
pub(super) fn observe_model(app: &mut AppState, model: &str) {
    app.session.observe_claude_model(model);
}

/// Omit inherited choices so native settings remain in charge.
pub(super) fn prompt_choices(
    app: &AppState,
    model: Option<String>,
    effort: Option<String>,
) -> (Option<String>, Option<String>) {
    app.session.claude_prompt_choices(model, effort)
}

/// Remember the initial values when the user opens Claude settings.
pub(super) fn begin_settings_edit(app: &mut AppState) {
    if app.view.claude_settings_edit.is_none() {
        let choices = (
            app.model_settings_model_value().to_string(),
            app.model_settings_effort_value().to_string(),
        );
        app.view.claude_settings_edit = Some(choices);
    }
}

/// Pass only the editor's captured values to the session's inheritance rules.
pub(super) fn prepare_saved_choices(app: &mut AppState, defaults: &mut RuntimeDefaults) {
    app.session
        .prepare_saved_claude_choices(app.view.claude_settings_edit.as_ref(), defaults);
}
