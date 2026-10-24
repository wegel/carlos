//! Save only the user's tool-output mode; Auto's size-based choice stays local.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use super::tool_output_state::ToolOutputMode;

fn preferences_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("carlos/ui-settings.json"))
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
    let parent = path.parent().context("invalid UI settings path")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let mut value = fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    value["tool_output"] = json!(mode.name());
    let temporary = parent.join(format!(".ui-settings-{}.json", std::process::id()));
    fs::write(&temporary, serde_json::to_vec_pretty(&value)?).context("write UI settings")?;
    fs::rename(&temporary, path).context("save UI settings")
}
