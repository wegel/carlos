//! Save tool-output choices and transcript themes for each screen size.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use super::layout_state::{ThemePreferences, TranscriptTheme};

use super::tool_output_state::ToolOutputMode;

fn preferences_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("carlos/ui-settings.json"))
}

/// Keep automatic labels optional while allowing an explicit startup override.
pub(super) fn load_summary_enabled() -> bool {
    carlos_core::ui_preferences::load_summary_enabled()
}

pub(super) fn save_summary_enabled(enabled: bool) -> Result<()> {
    carlos_core::ui_preferences::save_summary_enabled(enabled)
}

/// Read a saved choice, defaulting to Auto for missing or invalid settings.
pub(super) fn load_mode() -> ToolOutputMode {
    preferences_path()
        .map(|path| load_mode_from(&path))
        .unwrap_or_default()
}

/// Read a UI settings file without consulting process-wide environment variables.
pub(super) fn load_mode_from(path: &Path) -> ToolOutputMode {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|value| ToolOutputMode::parse(value.get("tool_output")?.as_str()?))
        .unwrap_or_default()
}

/// Save an explicit choice for later Carlos processes.
pub(super) fn save_mode(mode: ToolOutputMode) -> Result<()> {
    let path = preferences_path().context("cannot determine config directory")?;
    save_mode_to(&path, mode)
}

/// Replace the settings file atomically so readers never see half a write.
pub(super) fn save_mode_to(path: &Path, mode: ToolOutputMode) -> Result<()> {
    save_fields(path, &[("tool_output", json!(mode.name()))])
}

/// Read per-screen themes, migrating the former global layout choice.
pub(super) fn load_themes() -> ThemePreferences {
    preferences_path()
        .map(|path| load_themes_from(&path))
        .unwrap_or_default()
}

/// Read theme preferences without consulting process-wide environment variables.
pub(super) fn load_themes_from(path: &Path) -> ThemePreferences {
    let value = fs::read(path)
        .ok()
        .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
        .unwrap_or(Value::Null);
    let legacy = ThemePreferences::from_legacy(value["layout"].as_str());
    ThemePreferences {
        desktop: value["themes"]["desktop"]
            .as_str()
            .and_then(TranscriptTheme::parse)
            .unwrap_or(legacy.desktop),
        phone: value["themes"]["phone"]
            .as_str()
            .and_then(TranscriptTheme::parse)
            .unwrap_or(legacy.phone),
    }
}

/// Save the output preference and both themes when the settings panel closes.
pub(super) fn save_appearance(mode: ToolOutputMode, themes: ThemePreferences) -> Result<()> {
    let path = preferences_path().context("cannot determine config directory")?;
    save_appearance_to(&path, mode, themes)
}

/// Save per-screen themes atomically and replace the old global field.
pub(super) fn save_appearance_to(
    path: &Path,
    mode: ToolOutputMode,
    themes: ThemePreferences,
) -> Result<()> {
    save_fields(
        path,
        &[
            ("tool_output", json!(mode.name())),
            (
                "themes",
                json!({"desktop":themes.desktop.name(),"phone":themes.phone.name()}),
            ),
        ],
    )
}

fn save_fields(path: &Path, fields: &[(&str, Value)]) -> Result<()> {
    carlos_core::ui_preferences::save_fields_to(path, fields)
}
