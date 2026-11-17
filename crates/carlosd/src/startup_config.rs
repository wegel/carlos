//! Read optional private daemon settings before opening either listener.

use std::fs::OpenOptions;
use std::io::Read;
use std::net::SocketAddr;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use anyhow::{ensure, Context, Result};
use serde::Deserialize;

/// Select an explicit listener and optional exact browser Origin header.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Settings {
    pub(crate) listen: Option<SocketAddr>,
    pub(crate) allow_origin: Option<String>,
}

/// Load a bounded private file, or keep network access disabled when none was selected.
pub(crate) fn read(path: Option<&Path>) -> Result<Settings> {
    let Some(path) = path else {
        return Ok(Settings::default());
    };
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .with_context(|| format!("open daemon configuration {}", path.display()))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.permissions().mode() & 0o077 == 0,
        "daemon configuration must be a private regular file owned by this user: {}",
        path.display()
    );
    let mut bytes = Vec::new();
    file.by_ref().take(4097).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 4096,
        "daemon configuration exceeds 4096 bytes"
    );
    serde_json::from_slice(&bytes)
        .with_context(|| format!("parse daemon configuration {}", path.display()))
}

#[cfg(test)]
#[path = "startup_config_tests.rs"]
mod tests;
