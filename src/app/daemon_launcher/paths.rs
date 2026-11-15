//! Resolve and check private host paths without creating another settings store.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};

pub(super) struct Paths {
    pub socket: PathBuf,
    pub state: PathBuf,
    pub log: PathBuf,
    lock: PathBuf,
}

impl Paths {
    pub fn discover() -> Result<Self> {
        Self::resolve(
            std::env::var_os("XDG_RUNTIME_DIR"),
            std::env::var_os("XDG_STATE_HOME"),
            std::env::var_os("HOME"),
        )
    }

    pub fn resolve(
        runtime: Option<OsString>,
        state: Option<OsString>,
        home: Option<OsString>,
    ) -> Result<Self> {
        let state = match state {
            Some(path) => PathBuf::from(path),
            None => PathBuf::from(home.context("HOME or XDG_STATE_HOME is required")?)
                .join(".local/state"),
        };
        let state = private_directory(&state.join("carlos/daemon"))?;
        let runtime = runtime
            .map(|path| PathBuf::from(path).join("carlos"))
            .unwrap_or_else(|| state.join("run"));
        let runtime = private_directory(&runtime)?;
        Ok(Self {
            socket: runtime.join("daemon.sock"),
            lock: runtime.join("launch.lock"),
            log: state.join("daemon.log"),
            state,
        })
    }

    pub fn lock(&self, deadline: Instant) -> Result<File> {
        let file = private_file(&self.lock)?;
        loop {
            if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
                return Ok(file);
            }
            let error = io::Error::last_os_error();
            if !matches!(
                error.raw_os_error(),
                Some(libc::EWOULDBLOCK) | Some(libc::EINTR)
            ) {
                return Err(error).context("lock daemon launcher");
            }
            if Instant::now() >= deadline {
                bail!("another terminal is still starting the Carlos daemon; try again");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

pub(super) fn private_file(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        bail!(
            "daemon file must be private, regular and owned by this user: {}",
            path.display()
        );
    }
    Ok(file)
}

fn private_directory(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        bail!(
            "daemon runtime and state directories must be absolute: {}",
            path.display()
        );
    }
    if !path.exists() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    let metadata = path.symlink_metadata()?;
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != uid
        || metadata.permissions().mode() & 0o077 != 0
    {
        bail!(
            "daemon directory must be private, owned by this user and not a symlink: {}",
            path.display()
        );
    }
    let path = path.canonicalize()?;
    for ancestor in path.ancestors() {
        let metadata = ancestor.metadata()?;
        let mode = metadata.permissions().mode();
        let sticky_root = metadata.uid() == 0 && mode & 0o1000 != 0;
        if metadata.uid() != 0 && metadata.uid() != uid || mode & 0o022 != 0 && !sticky_root {
            bail!(
                "daemon directory has an untrusted writable ancestor: {}",
                ancestor.display()
            );
        }
    }
    Ok(path)
}
