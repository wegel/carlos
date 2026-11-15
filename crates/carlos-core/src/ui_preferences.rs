//! Keep the existing tool-label flag and device fields in one atomically merged JSON file.

mod lock;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::runtime_preferences::{write_atomic, AtomicWriteError};

/// Find the existing UI settings file without creating a second preferences store.
pub fn path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("carlos/ui-settings.json"))
}

/// Honor the exact startup override before the saved flag, defaulting to enabled.
pub fn load_summary_enabled() -> bool {
    let override_value = env::var("CARLOS_TOOL_SUMMARIES").ok();
    load_summary_enabled_from(path().as_deref(), override_value.as_deref())
}

/// Resolve a saved flag and an optional explicit startup value without reading the environment.
pub fn load_summary_enabled_from(path: Option<&Path>, override_value: Option<&str>) -> bool {
    if let Some(value) = override_value {
        return matches!(value, "on" | "true" | "1");
    }
    path.and_then(|path| fs::read(path).ok())
        .and_then(|raw| serde_json::from_slice::<Value>(&raw).ok())
        .and_then(|value| value["ai_tool_labels"].as_bool())
        .unwrap_or(true)
}

/// Save the host default without replacing device themes, output choices or unknown fields.
pub fn save_summary_enabled(enabled: bool) -> Result<()> {
    let path = path().context("cannot determine config directory")?;
    save_fields_to(&path, &[("ai_tool_labels", json!(enabled))])
}

/// Serialize field edits across processes, reread under that lock and publish the complete file.
pub fn save_fields_to(path: &Path, fields: &[(&str, Value)]) -> Result<()> {
    let destination = lock::destination(path)?;
    let _guard = lock::acquire(&destination)?;
    let mut value = fs::read(&destination)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    for (key, setting) in fields {
        value[*key] = setting.clone();
        if *key == "themes" {
            value
                .as_object_mut()
                .expect("settings object")
                .remove("layout");
        }
    }
    publish(&destination, &serde_json::to_vec_pretty(&value)?)
}

fn publish(destination: &Path, bytes: &[u8]) -> Result<()> {
    match write_atomic(destination, bytes) {
        Ok(()) => Ok(()),
        Err(AtomicWriteError::Prepare(error)) => Err(error).context("create UI settings directory"),
        Err(AtomicWriteError::Write(error)) => Err(error).context("write UI settings"),
        Err(AtomicWriteError::Publish(error)) => Err(error).context("save UI settings"),
    }
}

#[cfg(test)]
#[path = "ui_preferences_tests.rs"]
mod tests;
