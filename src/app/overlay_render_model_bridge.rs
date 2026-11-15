//! Lend existing embedded settings values to the session-independent painter.

use super::overlay_render_model_settings::{settings_cursor_position, SettingsFrame};
use crate::app::state::{AppState, ModelSettingsField};
use crate::app::view_models::TerminalSize;

pub(super) fn frame(app: &AppState) -> SettingsFrame<'_> {
    let selected = app.view.runtime_settings.model_settings_field;
    let fields = app
        .view
        .runtime_settings
        .model_settings_fields(app.session.runtime_settings());
    let help = app
        .view
        .codex_settings
        .editor_error
        .as_deref()
        .unwrap_or(match selected {
            ModelSettingsField::DictationEndpoint => "Arrows/Enter toggle, Esc close",
            ModelSettingsField::DesktopTheme => "Theme for wide screens",
            ModelSettingsField::PhoneTheme => "Theme for small screens",
            ModelSettingsField::Context => app.view.runtime_settings.context.help(),
            _ => "Tab field, arrows, Enter save",
        });
    SettingsFrame {
        fields: fields
            .into_iter()
            .map(|field| (field, field_value(app, field)))
            .collect(),
        selected,
        help,
    }
}

pub(in crate::app) fn model_settings_cursor_position(
    app: &AppState,
    size: TerminalSize,
) -> (usize, usize) {
    settings_cursor_position(&frame(app), size)
}

fn field_value(app: &AppState, field: ModelSettingsField) -> &str {
    match field {
        ModelSettingsField::Model => app.model_settings_model_value(),
        ModelSettingsField::Effort => app.model_settings_effort_value(),
        ModelSettingsField::Summary => app.model_settings_summary_value(),
        ModelSettingsField::Context if app.view.runtime_settings.context.attached => {
            "Server default"
        }
        ModelSettingsField::Context => &app.view.runtime_settings.context.draft,
        ModelSettingsField::DictationEndpoint => app.dictation_endpoint_mode_label(),
        ModelSettingsField::DesktopTheme => app.view.layout.draft.desktop.label(),
        ModelSettingsField::PhoneTheme => app.view.layout.draft.phone.label(),
        ModelSettingsField::ToolLabels => {
            if app.view.summary_enabled_draft {
                "On"
            } else {
                "Off"
            }
        }
        ModelSettingsField::ToolOutput => app
            .view
            .tool_output
            .settings_label(app.view.runtime_settings.model_settings_tool_output),
    }
}
