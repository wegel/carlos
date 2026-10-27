//! Locate summary settings and keep model invocation choices outside transcript code.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// Select a built-in Codex worker or an executable accepting JSON on stdin.
#[derive(Clone, Debug)]
pub(crate) struct Config {
    pub executable: String,
    pub arguments: Vec<String>,
    pub model: String,
    pub codex: bool,
    pub timeout: Duration,
    pub state: PathBuf,
}

/// Place generated state under XDG_STATE_HOME, separate from user configuration.
pub(crate) fn state_path() -> Result<PathBuf> {
    let base = env::var_os("XDG_STATE_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|s| PathBuf::from(s).join(".local/state")))
        .context("cannot locate Carlos state directory")?;
    Ok(base.join("carlos"))
}

/// Use the existing XDG configuration directory for provider settings.
pub(crate) fn config_path() -> Option<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|s| PathBuf::from(s).join(".config")))?;
    Some(base.join("carlos/tool-summaries.json"))
}

impl Config {
    /// Read optional provider choices only on the background worker.
    pub fn load() -> Result<Self> {
        let value = match config_path().filter(|p| p.exists()) {
            Some(path) => serde_json::from_slice::<Value>(&fs::read(path)?)?,
            None => Value::Null,
        };
        let provider = value["provider"].as_str().unwrap_or("codex");
        if !matches!(provider, "codex" | "command") {
            bail!("summary provider must be codex or command");
        }
        if provider == "command" && value["executable"].as_str().is_none_or(str::is_empty) {
            bail!("command summary provider requires an executable");
        }
        let arguments = value["arguments"]
            .as_array()
            .map(|values| {
                values
                    .iter()
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .context("summary argument must be a string")
                    })
                    .collect()
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            executable: value["executable"].as_str().unwrap_or("codex").to_owned(),
            arguments,
            model: value["model"].as_str().unwrap_or("gpt-6-luna").to_owned(),
            codex: provider == "codex",
            timeout: Duration::from_secs(
                value["timeout_seconds"]
                    .as_u64()
                    .unwrap_or(45)
                    .clamp(1, 120),
            ),
            state: state_path()?,
        })
    }
}
