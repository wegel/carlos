//! Keyboard edits and saves for the Ctrl+M settings panel.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::input_events::TerminalEventResult;
use crate::app::notifications::is_ctrl_char;
use crate::app::persist_runtime_defaults;
use crate::app::state::{AppState, ModelSettingsField};
use crate::app::tool_output_preferences::save_appearance;
use crate::backend::{BackendClient, BackendKind};

pub(super) fn handle_model_settings_key(
    client: &dyn BackendClient,
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
) -> TerminalEventResult {
    if client.kind() == BackendKind::Claude {
        crate::app::claude_models::begin_settings_edit(app);
    }
    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => return TerminalEventResult::Quit,
        (KeyCode::Esc, _) => app.close_model_settings(),
        (KeyCode::Tab, _) | (KeyCode::Down, _) => app.model_settings_move_field(true),
        (KeyCode::BackTab, _) | (KeyCode::Up, _) => app.model_settings_move_field(false),
        (KeyCode::Left, _) => cycle(app, -1),
        (KeyCode::Right, _) => cycle(app, 1),
        (KeyCode::Backspace, _)
            if matches!(app.runtime.model_settings_field, ModelSettingsField::Model)
                && !app.model_settings_has_model_choices() =>
        {
            app.model_settings_backspace();
        }
        (KeyCode::Enter, _)
            if matches!(
                app.runtime.model_settings_field,
                ModelSettingsField::DictationEndpoint
            ) =>
        {
            app.toggle_dictation_endpoint_mode();
        }
        (KeyCode::Enter, _) => save_settings(client, app),
        (KeyCode::Char(ch), mods)
            if !mods.contains(KeyModifiers::CONTROL)
                && !mods.contains(KeyModifiers::ALT)
                && matches!(app.runtime.model_settings_field, ModelSettingsField::Model)
                && !app.model_settings_has_model_choices() =>
        {
            app.model_settings_insert_char(ch);
        }
        _ => {}
    }
    if !app.runtime.show_model_settings {
        app.claude.selection.editing = None;
    }
    if app.codex.enabled {
        crate::app::codex_settings_draft::refresh_editor(app);
    }
    TerminalEventResult::Continue { needs_draw: true }
}

fn cycle(app: &mut AppState, dir: isize) {
    match app.runtime.model_settings_field {
        ModelSettingsField::Model => app.model_settings_cycle_model(dir),
        ModelSettingsField::Effort => app.model_settings_cycle_effort(dir),
        ModelSettingsField::Summary => app.model_settings_cycle_summary(dir),
        ModelSettingsField::DictationEndpoint => app.toggle_dictation_endpoint_mode(),
        ModelSettingsField::DesktopTheme => {
            app.layout.draft.desktop = app.layout.draft.desktop.cycle(dir)
        }
        ModelSettingsField::PhoneTheme => {
            app.layout.draft.phone = app.layout.draft.phone.cycle(dir)
        }
        ModelSettingsField::ToolLabels => app.summaries.draft = !app.summaries.draft,
        ModelSettingsField::ToolOutput => {
            app.runtime.model_settings_tool_output =
                app.runtime.model_settings_tool_output.cycle(dir);
        }
    }
}

fn save_settings(client: &dyn BackendClient, app: &mut AppState) {
    if app.codex.enabled && app.codex.closed {
        app.codex.settings.editor_error =
            Some("Codex is disconnected; F5 reconnects. Choices remain in this editor".into());
        return;
    }
    if client.kind() == BackendKind::Codex
        && !crate::app::codex_settings_draft::validate_editor(app)
    {
        return;
    }
    let before = app.next_turn_runtime_settings();
    let pending = (
        app.runtime.pending_model.clone(),
        app.runtime.pending_effort.clone(),
        app.runtime.pending_summary.clone(),
    );
    let mut defaults = app.apply_model_settings();
    if client.kind() == BackendKind::Claude {
        crate::app::claude_models::prepare_saved_choices(app, &mut defaults);
    }
    if client.kind() == BackendKind::Codex {
        crate::app::codex_settings_draft::normalize_saved(app, &mut defaults);
        if before != app.next_turn_runtime_settings() {
            app.codex.settings.persist_next = true;
            let mut desired = serde_json::json!({});
            for (key, value) in [
                ("model", &defaults.model),
                ("effort", &defaults.effort),
                ("summary", &defaults.summary),
            ] {
                if let Some(value) = value {
                    desired[key] = serde_json::json!(value);
                }
            }
            crate::app::codex_settings::save_values(client, app, desired);
        } else {
            app.queue_runtime_settings(pending.0, pending.1, pending.2);
        }
    }
    match client.apply_runtime_settings(defaults.model.as_deref(), defaults.effort.as_deref()) {
        Ok(true) => {
            app.mark_runtime_settings_applied();
            app.set_status("Claude settings applied");
            crate::app::claude_context::invalidate(app);
        }
        Ok(false) => {}
        Err(error) => app.set_status(format!("Could not apply live settings: {error}")),
    }
    if let Err(error) =
        crate::app::tool_output_preferences::save_summary_enabled(app.summaries.enabled)
    {
        app.set_status(format!("Could not save AI tool labels: {error}"));
    }
    if let Err(error) = save_appearance(app.tool_output.mode, app.layout.themes) {
        app.append_message(
            crate::app::Role::System,
            format!("Could not save appearance choices: {error}"),
        );
    }
    if let Err(err) = if client.kind() == BackendKind::Codex {
        Ok(())
    } else {
        persist_runtime_defaults(&defaults)
    } {
        app.set_status(format!("saved for next turn; default save failed: {err}"));
    }
}
