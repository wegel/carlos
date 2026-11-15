//! Hold one stable sibling lock while two processes merge fields into the same settings file.

use std::fs::{self, File, OpenOptions};
use std::io;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};

pub(super) fn destination(path: &Path) -> Result<PathBuf> {
    let parent = path.parent().context("invalid UI settings path")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    match fs::symlink_metadata(path) {
        Ok(_) => {
            let destination = path.canonicalize().context("resolve UI settings path")?;
            if !fs::metadata(&destination)?.is_file() {
                bail!("UI settings target is not a regular file");
            }
            Ok(destination)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(parent.canonicalize()?.join(
            path.file_name()
                .context("UI settings path has no filename")?,
        )),
        Err(error) => Err(error).context("inspect UI settings path"),
    }
}

#[cfg(unix)]
pub(super) fn acquire(path: &Path) -> Result<File> {
    let lock_path = path.with_file_name(format!(
        ".{}.lock",
        path.file_name()
            .context("UI settings path has no filename")?
            .to_string_lossy()
    ));
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(lock_path)
        .context("open UI settings lock")?;
    if !file.metadata()?.is_file() {
        bail!("UI settings lock is not a regular file");
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        // Never remove this file: another process may still hold its original inode.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(file);
        }
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::WouldBlock {
            return Err(error).context("lock UI settings");
        }
        if Instant::now() >= deadline {
            bail!("UI settings remained locked for two seconds");
        }
        thread::sleep(Duration::from_millis(5));
    }
}

#[cfg(not(unix))]
pub(super) fn acquire(_path: &Path) -> Result<File> {
    bail!("UI settings require an operating-system file lock on this platform")
}
