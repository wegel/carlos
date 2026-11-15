//! Connect or start one matching private host daemon without owning native sessions here.

mod companion;
mod paths;
mod probe;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use carlos_client::Client;

use self::paths::Paths;
use crate::app::cli::CliOptions;
use crate::app::remote::{self, Transport};

pub(super) fn run(options: &CliOptions) -> Result<()> {
    let paths = Paths::discover()?;
    let executable = std::env::current_exe().context("locate Carlos executable")?;
    let endpoint = options
        .codex_socket
        .as_deref()
        .map(normalize_endpoint)
        .transpose()?;
    let (client, transport) = prepare(
        &paths,
        &executable,
        endpoint.as_deref(),
        Instant::now() + Duration::from_secs(5),
    )?;
    let mut options = options.clone();
    options.codex_socket = endpoint;
    remote::run_connected(&paths.socket, &options, client, transport)
}

/// Greet the managed daemon without launching or restoring a native session.
pub(super) fn connect_local() -> Result<(Client, Transport)> {
    let paths = Paths::discover()?;
    let executable = std::env::current_exe().context("locate Carlos executable")?;
    prepare(
        &paths,
        &executable,
        None,
        Instant::now() + Duration::from_secs(5),
    )
}

fn prepare(
    paths: &Paths,
    executable: &Path,
    endpoint: Option<&Path>,
    deadline: Instant,
) -> Result<(Client, Transport)> {
    if let Some(connected) = probe::connect(&paths.socket, endpoint, deadline)? {
        return Ok(connected);
    }
    let _lock = paths.lock(deadline)?;
    if let Some(connected) = probe::connect(&paths.socket, endpoint, deadline)? {
        return Ok(connected);
    }
    let daemon = companion::locate(executable)?;
    companion::check(&daemon, deadline)?;
    let mut child = companion::start(&daemon, paths, endpoint)?;
    let outcome = wait_started(paths, endpoint, deadline, &mut child);
    match outcome {
        Ok(connected) => {
            // Reap an explicit daemon stop while this terminal remains open; never signal it.
            std::thread::Builder::new()
                .name("daemon-reaper".into())
                .spawn(move || {
                    let _ = child.wait();
                })
                .context("start daemon child reaper")?;
            Ok(connected)
        }
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(error)
        }
    }
}

fn wait_started(
    paths: &Paths,
    endpoint: Option<&Path>,
    deadline: Instant,
    child: &mut std::process::Child,
) -> Result<(Client, Transport)> {
    loop {
        if let Some(connected) = probe::connect(&paths.socket, endpoint, deadline)? {
            return Ok(connected);
        }
        if let Some(status) = child.try_wait()? {
            bail!(
                "Carlos daemon exited during startup ({status}); see {}",
                paths.log.display()
            );
        }
        if Instant::now() >= deadline {
            bail!(
                "Carlos daemon did not start within 5 seconds; see {}",
                paths.log.display()
            );
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn normalize_endpoint(path: &Path) -> Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    // Resolve existing symlink aliases while allowing the native server to appear later.
    Ok(absolute.canonicalize().unwrap_or(absolute))
}
