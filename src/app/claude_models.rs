//! Bridge Claude metadata to local settings and edited-prompt widgets.

use carlos_core::session::runtime_settings::RuntimeDefaults;

use super::state::AppState;

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
