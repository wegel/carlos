//! Read and atomically replace the shared Carlos runtime-defaults file.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tempfile::NamedTempFile;

use crate::session::runtime_settings::RuntimeDefaults;

/// Find the existing shared defaults file under XDG_CONFIG_HOME or HOME/.config.
pub fn path() -> Option<PathBuf> {
    path_from(env::var_os("XDG_CONFIG_HOME"), env::var_os("HOME"))
}

/// Read saved choices, using empty choices when the path or file is unavailable.
pub fn load() -> RuntimeDefaults {
    path().map_or_else(RuntimeDefaults::default, |path| load_from(&path))
}

/// Read each nonempty string choice, ignoring unreadable files and malformed JSON.
pub fn load_from(path: &Path) -> RuntimeDefaults {
    let Ok(contents) = fs::read_to_string(path) else {
        return RuntimeDefaults::default();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&contents) else {
        return RuntimeDefaults::default();
    };
    let choice = |key| {
        parsed
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    RuntimeDefaults {
        model: choice("model"),
        effort: choice("effort"),
        summary: choice("summary"),
    }
}

/// Atomically save all choices to the shared defaults file.
pub fn save(defaults: &RuntimeDefaults) -> io::Result<()> {
    let path = path().ok_or_else(|| io::Error::other("cannot determine config directory"))?;
    save_to(&path, defaults)
}

/// Publish one complete JSON file, preserving links to existing regular files.
pub fn save_to(path: &Path, defaults: &RuntimeDefaults) -> io::Result<()> {
    let destination = destination(path)?;
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "invalid runtime defaults path")
    })?;
    fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(&json!({
        "model": defaults.model,
        "effort": defaults.effort,
        "summary": defaults.summary,
    }))?;
    publish(&destination, parent, &bytes)
}

fn publish(destination: &Path, parent: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut temporary = NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    require_regular_or_absent(destination)?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

fn path_from(config: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    let base = config
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| home.map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("carlos").join("runtime-defaults.json"))
}

fn destination(path: &Path) -> io::Result<PathBuf> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::canonicalize(path).map_err(|error| {
                io::Error::new(
                    error.kind(),
                    format!("resolve runtime defaults link {}: {error}", path.display()),
                )
            })?;
            require_regular_or_absent(&target)?;
            Ok(target)
        }
        Ok(_) => {
            require_regular_or_absent(path)?;
            Ok(path.to_owned())
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(path.to_owned()),
        Err(error) => Err(error),
    }
}

fn require_regular_or_absent(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "runtime defaults target is not a regular file: {}",
                path.display()
            ),
        )),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "runtime_preferences_tests.rs"]
mod tests;
