//! Store a separate context choice for each backend without editing native CLI settings.

use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::backend::BackendKind;

/// Accept the token range shared by the settings editor and Claude's native control.
pub(crate) fn validate(tokens: u64) -> Result<u64> {
    if !(100_000..=1_000_000).contains(&tokens) {
        bail!("Choose Auto or 100k to 1M tokens");
    }
    Ok(tokens)
}

/// Parse a token count or a whole number with a k or M suffix.
pub(crate) fn parse(text: &str) -> Result<Option<u64>> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let (number, scale) = match text.as_bytes().last() {
        Some(b'k' | b'K') => (&text[..text.len() - 1], 1_000),
        Some(b'm' | b'M') => (&text[..text.len() - 1], 1_000_000),
        _ => (text, 1),
    };
    let tokens = number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(scale))
        .context("Choose Auto or a token count, such as 500k")?;
    validate(tokens).map(Some)
}

/// Format a stored choice for the settings editor.
pub(crate) fn label(tokens: Option<u64>) -> String {
    match tokens {
        None => "Auto".into(),
        Some(1_000_000) => "1M".into(),
        Some(tokens) if tokens % 1_000 == 0 => format!("{}k", tokens / 1_000),
        Some(tokens) => tokens.to_string(),
    }
}

/// Read Carlos's saved override; a missing file means the backend chooses its default.
pub(crate) fn load(backend: BackendKind) -> Result<Option<u64>> {
    load_from(&path(backend)?)
}

/// Save one backend's choice atomically without touching the other backends.
pub(crate) fn save(backend: BackendKind, tokens: Option<u64>) -> Result<()> {
    save_to(&path(backend)?, tokens)
}

fn path(backend: BackendKind) -> Result<PathBuf> {
    let base = env::var_os("XDG_CONFIG_HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .context("Cannot find Carlos's settings folder")?;
    let name = match backend {
        BackendKind::Claude => "claude",
        BackendKind::Codex => "codex",
        BackendKind::Grok => "grok",
    };
    Ok(base.join("carlos").join(format!("{name}-context.json")))
}

fn load_from(path: &Path) -> Result<Option<u64>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("Read {}", path.display())),
    };
    let value: Value = serde_json::from_slice(&bytes).context("Read Carlos context settings")?;
    match value.get("tokens") {
        Some(Value::Null) => Ok(None),
        Some(value) => validate(value.as_u64().context("Invalid saved context size")?).map(Some),
        None => bail!("Saved context settings have no token count"),
    }
}

fn save_to(path: &Path, tokens: Option<u64>) -> Result<()> {
    if let Some(tokens) = tokens {
        validate(tokens)?;
    }
    fs::create_dir_all(path.parent().context("Invalid context settings path")?)?;
    let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    fs::write(
        &temporary,
        serde_json::to_vec_pretty(&json!({"tokens":tokens}))?,
    )
    .context("Write Carlos context settings")?;
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(temporary);
        return Err(error).context("Save Carlos context settings");
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/context_preferences_tests.rs"]
mod tests;
