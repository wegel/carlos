//! Keep settings keystrokes and unsaved values on the terminal device.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::settings::catalog_runtime;
use super::transport::Transport;
use super::Device;
use crate::app::notifications::is_ctrl_char;
use crate::app::runtime_settings_state::ModelSettingsField;

impl Device {
    pub(super) fn settings_key(
        &mut self,
        link: Option<&mut Transport>,
        key: KeyEvent,
    ) -> Result<bool> {
        if is_ctrl_char(key.code, key.modifiers, 'm') {
            if self.view.runtime_settings.show_model_settings {
                self.close_settings();
            } else {
                self.open_settings(link)?;
            }
            return Ok(true);
        }
        if !self.view.runtime_settings.show_model_settings {
            return Ok(false);
        }
        if key.code == KeyCode::Esc {
            self.close_settings();
            return Ok(true);
        }
        if key.code == KeyCode::F(6) {
            self.refresh_settings(link)?;
            return Ok(true);
        }
        if self.settings.submitted.is_some() || self.settings_refreshing() {
            return Ok(true);
        }
        if self.settings.catalog.is_some() {
            if key.code == KeyCode::Enter
                && self.view.runtime_settings.model_settings_field
                    != ModelSettingsField::DictationEndpoint
            {
                self.save_settings(link)?;
            } else {
                self.edit_settings_key(key);
            }
        }
        Ok(true)
    }

    fn edit_settings_key(&mut self, key: KeyEvent) {
        if self.edit_context_key(key) {
            return;
        }
        let catalog = self
            .settings
            .catalog
            .as_ref()
            .expect("captured editor catalog");
        match key.code {
            KeyCode::Tab | KeyCode::Down => self
                .view
                .runtime_settings
                .model_settings_move_field(catalog_runtime(catalog), true),
            KeyCode::BackTab | KeyCode::Up => self
                .view
                .runtime_settings
                .model_settings_move_field(catalog_runtime(catalog), false),
            KeyCode::Left => self.cycle_settings(-1),
            KeyCode::Right => self.cycle_settings(1),
            KeyCode::Enter
                if self.view.runtime_settings.model_settings_field
                    == ModelSettingsField::DictationEndpoint =>
            {
                self.view.toggle_dictation_endpoint_mode()
            }
            KeyCode::Backspace
                if catalog.models.is_empty()
                    && self.view.runtime_settings.model_settings_field
                        == ModelSettingsField::Model =>
            {
                self.view.runtime_settings.model_settings_backspace()
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && catalog.models.is_empty()
                    && self.view.runtime_settings.model_settings_field
                        == ModelSettingsField::Model =>
            {
                self.view.runtime_settings.model_settings_insert_char(ch)
            }
            _ => {}
        }
    }

    fn cycle_settings(&mut self, direction: isize) {
        let editor = &mut self.view.runtime_settings;
        match editor.model_settings_field {
            ModelSettingsField::Model => {
                let runtime = catalog_runtime(self.settings.catalog.as_ref().expect("catalog"));
                editor.model_settings_cycle_model(runtime, direction);
                editor.refresh_native_editor(runtime, false);
            }
            ModelSettingsField::Effort => editor.model_settings_cycle_effort(direction),
            ModelSettingsField::Summary => editor.model_settings_cycle_summary(direction),
            ModelSettingsField::Context => {
                editor.context.cycle(direction);
                self.settings.context.edited = true;
            }
            ModelSettingsField::DictationEndpoint => self.view.toggle_dictation_endpoint_mode(),
            ModelSettingsField::DesktopTheme => {
                self.view.layout.draft.desktop = self.view.layout.draft.desktop.cycle(direction)
            }
            ModelSettingsField::PhoneTheme => {
                self.view.layout.draft.phone = self.view.layout.draft.phone.cycle(direction)
            }
            ModelSettingsField::ToolLabels => {
                self.view.summary_enabled_draft = !self.view.summary_enabled_draft
            }
            ModelSettingsField::ToolOutput => {
                editor.model_settings_tool_output =
                    editor.model_settings_tool_output.cycle(direction)
            }
        }
    }

    pub(super) fn settings_paste(&mut self, text: &str) {
        if self
            .settings
            .catalog
            .as_ref()
            .is_some_and(|catalog| catalog.models.is_empty())
            && self.view.runtime_settings.model_settings_field == ModelSettingsField::Model
        {
            self.view
                .runtime_settings
                .model_settings_model_input
                .push_str(text);
        } else if self.view.runtime_settings.model_settings_field == ModelSettingsField::Context {
            self.view.runtime_settings.context.insert(text);
            self.settings.context.edited = true;
        }
    }

    fn edit_context_key(&mut self, key: KeyEvent) -> bool {
        if self.view.runtime_settings.model_settings_field != ModelSettingsField::Context {
            return false;
        }
        let editor = &mut self.view.runtime_settings.context;
        match key.code {
            KeyCode::Backspace => editor.backspace(),
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                editor.insert(&ch.to_string());
            }
            _ => return false,
        }
        self.settings.context.edited = true;
        true
    }
}
