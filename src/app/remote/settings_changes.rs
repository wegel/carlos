//! Convert private editor changes to explicit field choices without pinning displayed defaults.

use anyhow::{bail, Result};
use carlos_core::session::runtime_settings::RuntimeDefaults;
use carlos_proto::session::Backend;
use carlos_proto::settings::{SettingValue, SettingsChange};

use super::settings::Catalog;
use crate::app::runtime_settings_state::RuntimeSettingsView;

pub(super) fn choices(editor: &RuntimeSettingsView, catalog: &Catalog) -> RuntimeDefaults {
    let mut choices = editor.saved_choices(catalog.runtime());
    editor.normalize_native_choices(catalog.runtime(), &mut choices);
    choices
}

pub(super) fn change(
    editor: &RuntimeSettingsView,
    catalog: &Catalog,
    initial: &RuntimeDefaults,
    backend: Backend,
) -> Result<SettingsChange> {
    let change = capture(editor, catalog, initial, backend);
    if matches!(&change.model, Some(SettingValue::Value(model)) if model.trim().is_empty()) {
        bail!("Choose a nonempty model before saving");
    }
    Ok(change)
}

pub(super) fn capture(
    editor: &RuntimeSettingsView,
    catalog: &Catalog,
    initial: &RuntimeDefaults,
    backend: Backend,
) -> SettingsChange {
    let edited = choices(editor, catalog);
    let model_changed = edited.model != initial.model;
    let model = changed(edited.model, &initial.model, backend, false);
    let effort = if model_changed && edited.effort.is_some() {
        changed(edited.effort, &None, backend, true)
    } else {
        changed(edited.effort, &initial.effort, backend, true)
    };
    let summary = edited
        .summary
        .filter(|value| initial.summary.as_ref() != Some(value));
    SettingsChange {
        model,
        effort,
        summary: summary.map(SettingValue::Value),
    }
}

fn changed(
    value: Option<String>,
    initial: &Option<String>,
    backend: Backend,
    effort: bool,
) -> Option<SettingValue> {
    if &value == initial {
        return None;
    }
    match value {
        Some(value) if effort && backend == Backend::Claude && value == "auto" => {
            Some(SettingValue::Inherit)
        }
        Some(value) => Some(SettingValue::Value(value)),
        None if backend == Backend::Claude => Some(SettingValue::Inherit),
        None if effort => None,
        None => Some(SettingValue::Value(String::new())),
    }
}

/// Reapply only edited fields over a fresh catalog; untouched fields follow newer native facts.
pub(super) fn restore(editor: &mut RuntimeSettingsView, catalog: &Catalog, change: SettingsChange) {
    if let Some(model) = change.model {
        let model = match model {
            SettingValue::Value(model) => model,
            SettingValue::Inherit => String::new(),
        };
        editor.model_settings_model_index = catalog
            .models
            .iter()
            .position(|item| item.model == model)
            .unwrap_or(usize::MAX);
        editor.model_settings_model_input = model;
        if editor.model_settings_model_index != usize::MAX {
            editor.model_settings_cycle_model(catalog.runtime(), 0);
        }
    }
    if let Some(effort) = change.effort {
        let value = match effort {
            SettingValue::Value(value) => value,
            SettingValue::Inherit => "auto".into(),
        };
        editor.model_settings_effort_index =
            choose(&mut editor.model_settings_effort_options, value);
    }
    if let Some(SettingValue::Value(summary)) = change.summary {
        editor.model_settings_summary_index =
            choose(&mut editor.model_settings_summary_options, summary);
    }
    editor.refresh_native_editor(catalog.runtime(), false);
}

fn choose(options: &mut Vec<String>, value: String) -> usize {
    if let Some(index) = options.iter().position(|option| option == &value) {
        return index;
    }
    options.push(value);
    options.len() - 1
}
