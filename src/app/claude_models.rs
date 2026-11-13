//! Bridge Claude metadata to local settings and edited-prompt widgets.

#[cfg(test)]
use serde_json::Value;

use super::state::AppState;
use super::RuntimeDefaults;

/// Validate native metadata before touching the saved edited-prompt queue.
#[cfg(test)]
pub(super) fn apply_capabilities(app: &mut AppState, capabilities: &Value) {
    let draft = super::claude_notifications::context(app);
    let draft = crate::session::claude::receive_update::ClaudeDraftContext {
        composer: app.view.input.lines(),
        ..draft
    };
    let mut update = Default::default();
    app.session
        .receive_claude_capabilities(capabilities, &draft, &mut update);
    let _ = app.apply_claude_update(update);
}

/// Fence context reads before recording a changed native model.
#[cfg(test)]
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
