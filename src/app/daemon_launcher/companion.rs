//! Check and start only the daemon shipped beside the running terminal executable.

use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use carlos_proto::identity::BuildInfo;

use super::paths::{private_file, Paths};

pub(super) fn locate(executable: &Path) -> Result<PathBuf> {
    let daemon = executable
        .parent()
        .context("Carlos executable has no parent")?
        .join("carlosd");
    if !daemon.is_file() {
        bail!("matching daemon binary is missing: {}; keep carlos and carlosd from the same package together", daemon.display());
    }
    Ok(daemon)
}

pub(super) fn check(daemon: &Path, deadline: Instant) -> Result<()> {
    let mut child = Command::new(daemon)
        .arg("--build-info")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("run companion daemon {}", daemon.display()))?;
    let outcome = read_identity(&mut child, deadline);
    if outcome.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let identity = outcome.with_context(|| {
        format!(
            "cannot identify companion daemon protocol: {}",
            daemon.display()
        )
    })?;
    if identity.protocol != carlos_proto::PROTOCOL_VERSION {
        bail!(
            "companion carlosd protocol {} is incompatible with terminal protocol {}",
            identity.protocol,
            carlos_proto::PROTOCOL_VERSION
        );
    }
    if crate::app::cli::env_flag_enabled("CARLOS_STRICT_DAEMON_BUILD")
        && identity != crate::build_info()
    {
        bail!("companion carlosd build differs from carlos; CARLOS_STRICT_DAEMON_BUILD requires an exact build match");
    }
    Ok(())
}

pub(super) fn start(daemon: &Path, paths: &Paths, endpoint: Option<&Path>) -> Result<Child> {
    let log = private_file(&paths.log).context("open private daemon log")?;
    let mut command = Command::new(daemon);
    command
        .arg("--socket")
        .arg(&paths.socket)
        .arg("--state-dir")
        .arg(&paths.state)
        .current_dir(&paths.state)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    if let Some(endpoint) = endpoint {
        command.arg("--codex-socket").arg(endpoint);
    }
    if let Some(config) = configuration()? {
        command.arg("--config").arg(config);
    }
    // setsid is async-signal-safe and prevents the terminal's hangup from owning the daemon.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    command.spawn().context("start colocated Carlos daemon")
}

fn configuration() -> Result<Option<PathBuf>> {
    let Some(directory) = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    else {
        return Ok(None);
    };
    if !directory.is_absolute() {
        bail!("daemon configuration directory must be absolute");
    }
    let path = directory.join("carlos/daemon.json");
    Ok(path.try_exists()?.then_some(path))
}

fn read_identity(child: &mut Child, deadline: Instant) -> Result<BuildInfo> {
    let stdout = child.stdout.as_mut().expect("piped build identity");
    let flags = unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(stdout.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
    {
        return Err(std::io::Error::last_os_error()).context("read companion build identity");
    }
    let mut output = Vec::new();
    let mut bytes = [0; 1024];
    loop {
        match child.stdout.as_mut().expect("stdout").read(&mut bytes) {
            Ok(0) => {
                if let Some(status) = child.try_wait()? {
                    if !status.success() {
                        bail!("companion carlosd --build-info failed: {status}");
                    }
                    return serde_json::from_slice(&output)
                        .context("companion carlosd returned invalid build identity");
                }
            }
            Ok(count) => {
                output.extend_from_slice(&bytes[..count]);
                if output.len() > 4096 {
                    bail!("companion build identity exceeds 4096 bytes");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error.into()),
        }
        if Instant::now() >= deadline {
            bail!(
                "companion carlosd did not report its build identity before the startup deadline"
            );
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}
