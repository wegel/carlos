//! Borrow private settings rows for the same painter used by the embedded terminal.

use super::Device;
use crate::app::overlay_render::SettingsFrame;
use crate::app::overlay_render::{draw_settings_frame, settings_cursor_position};
use crate::app::runtime_settings_state::ModelSettingsField;

impl Device {
    pub(super) fn draw_settings(
        &self,
        frame: &mut ratatui::Frame<'_>,
        size: crate::app::view_models::TerminalSize,
    ) -> bool {
        if !self.view.runtime_settings.show_model_settings {
            return false;
        }
        let settings = self.settings_frame();
        draw_settings_frame(frame.buffer_mut(), size, &settings);
        let (x, y) = settings_cursor_position(&settings, size);
        frame.set_cursor_position((x as u16, y as u16));
        true
    }

    pub(super) fn settings_frame(&self) -> SettingsFrame<'_> {
        let selected = self.view.runtime_settings.model_settings_field;
        let Some(catalog) = &self.settings.catalog else {
            return SettingsFrame {
                fields: Vec::new(),
                selected,
                help: self
                    .settings
                    .error
                    .as_deref()
                    .unwrap_or("Reading model catalog…"),
            };
        };
        SettingsFrame {
            fields: self
                .view
                .runtime_settings
                .model_settings_fields(catalog.runtime())
                .into_iter()
                .map(|field| (field, self.settings_field_value(field)))
                .collect(),
            selected,
            help: self
                .settings
                .error
                .as_deref()
                .unwrap_or(if self.settings_refreshing() {
                    "Refreshing backend facts; edited fields stay private"
                } else {
                    match selected {
                        ModelSettingsField::Context => self.settings_context_help(),
                        ModelSettingsField::DesktopTheme => "Theme for wide screens",
                        ModelSettingsField::PhoneTheme => "Theme for small screens",
                        ModelSettingsField::DictationEndpoint => "Arrows/Enter toggle, Esc close",
                        _ => "Tab field, arrows, Enter save",
                    }
                }),
        }
    }

    fn settings_field_value(&self, field: ModelSettingsField) -> &str {
        let editor = &self.view.runtime_settings;
        let catalog = self
            .settings
            .catalog
            .as_ref()
            .expect("captured editor catalog");
        match field {
            ModelSettingsField::Model => editor.model_settings_model_value(catalog.runtime()),
            ModelSettingsField::Effort => editor.model_settings_effort_value(),
            ModelSettingsField::Summary => editor.model_settings_summary_value(catalog.runtime()),
            ModelSettingsField::Context if editor.context.attached => "Server default",
            ModelSettingsField::Context => &editor.context.draft,
            ModelSettingsField::DictationEndpoint => self.view.dictation_endpoint_mode_label(),
            ModelSettingsField::DesktopTheme => self.view.layout.draft.desktop.label(),
            ModelSettingsField::PhoneTheme => self.view.layout.draft.phone.label(),
            ModelSettingsField::ToolLabels => {
                if self.view.summary_enabled_draft {
                    "On"
                } else {
                    "Off"
                }
            }
            ModelSettingsField::ToolOutput => self
                .view
                .tool_output
                .settings_label(editor.model_settings_tool_output),
        }
    }
}
