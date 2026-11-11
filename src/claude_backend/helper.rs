//! Locates the compiled controller and the user's installed native Claude executable.

use std::env;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use super::types::{ClaudeLaunchMode, ClaudeRuntimeSettings};

/// Find a packaged helper without building or downloading anything at launch.
pub(super) fn controller_executable() -> Result<PathBuf> {
    if let Some(path) = env::var_os("CARLOS_CLAUDE_CONTROLLER") {
        return checked_executable(PathBuf::from(path));
    }
    let executable = env::current_exe().context("find Carlos executable")?;
    let bin = executable
        .parent()
        .context("Carlos executable has no folder")?;
    let name = if cfg!(windows) {
        "carlos-claude-controller.exe"
    } else {
        "carlos-claude-controller"
    };
    let mut candidates = vec![bin.join(name), bin.join("../libexec/carlos").join(name)];
    if matches!(
        bin.file_name().and_then(|name| name.to_str()),
        Some("debug" | "release")
    ) {
        candidates.push(bin.join("../claude-controller").join(name));
    }
    for path in candidates {
        if path.is_file() {
            return checked_executable(path);
        }
    }
    bail!("Claude controller is missing. Build it with scripts/build-claude-controller.sh or set CARLOS_CLAUDE_CONTROLLER to the packaged helper")
}

/// Find the installed native Claude executable.
pub(super) fn native_executable() -> Result<PathBuf> {
    if let Some(path) = env::var_os("CARLOS_CLAUDE_EXECUTABLE") {
        return checked_executable(PathBuf::from(path));
    }
    native_on_path(&env::var_os("PATH").unwrap_or_default())
}

/// Search PATH while skipping files the OS cannot execute.
pub(super) fn native_on_path(path: &std::ffi::OsStr) -> Result<PathBuf> {
    for folder in env::split_paths(path) {
        let candidate = folder.join(if cfg!(windows) {
            "claude.exe"
        } else {
            "claude"
        });
        if candidate.is_file() {
            if let Ok(path) = checked_executable(candidate) {
                return Ok(path);
            }
        }
    }
    bail!("Native Claude executable is missing from PATH")
}

/// Build native session choices without forcing permission or model defaults.
pub(super) fn start_params(
    cwd: &Path,
    launch: &ClaudeLaunchMode,
    settings: &ClaudeRuntimeSettings,
) -> Result<Value> {
    let mut params = json!({ "executable": native_executable()?, "cwd": cwd.canonicalize()?,
        "settingSources": ["user", "project", "local"], "supportedDialogKinds": [], "perTaskStopAffordance": true });
    if let ClaudeLaunchMode::Resume(id) = launch {
        params["resume"] = json!(id);
    }
    if let Some(model) = &settings.model {
        params["model"] = json!(model);
    }
    if let Some(effort) = &settings.effort {
        params["effort"] = json!(effort);
    }
    Ok(params)
}

fn checked_executable(path: PathBuf) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("find executable {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.metadata()?.permissions().mode() & 0o111 == 0 {
            bail!("{} is not executable", path.display());
        }
    }
    Ok(path)
}
