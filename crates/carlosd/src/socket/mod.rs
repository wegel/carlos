//! Bind one same-user Unix endpoint without unlinking another listener or unsafe filesystem entry.

mod paths;
mod peer;

use std::fs::{File, OpenOptions};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};

use socket2::{Domain, SockAddr, Socket, Type};

pub use self::peer::verify_peer;

/// Hold both locks until the listener and its owned pathname are retired.
pub struct BoundSocket {
    listener: UnixListener,
    path: PathBuf,
    identity: (u64, u64),
    _socket_lock: File,
    _state_lock: File,
}

impl BoundSocket {
    /// Accept only private directories owned by this OS user and a proven stale socket.
    pub fn bind(socket: &Path, state: &Path) -> io::Result<Self> {
        let socket = paths::absolute(socket)?;
        let name = socket
            .file_name()
            .ok_or_else(|| io::Error::other("socket path has no filename"))?;
        let parent = paths::private_directory(socket.parent().expect("absolute socket parent"))?;
        let state = paths::private_directory(&paths::absolute(state)?)?;
        let path = parent.join(name);
        let mut lock_name = std::ffi::OsString::from(".");
        lock_name.push(name);
        lock_name.push(".lock");
        let socket_lock = lock(&parent.join(lock_name))?;
        let state_lock = lock(&state.join(".daemon.lock"))?;
        remove_stale(&path)?;
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let metadata = path.symlink_metadata()?;
        listener.set_nonblocking(true)?;
        Ok(Self {
            listener,
            path,
            identity: (metadata.dev(), metadata.ino()),
            _socket_lock: socket_lock,
            _state_lock: state_lock,
        })
    }

    /// Borrow the listener for the dedicated accept worker.
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }
    /// Report the actual canonical endpoint used by the server.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn state_guard(&self) -> io::Result<File> {
        self._state_lock.try_clone()
    }
}

impl Drop for BoundSocket {
    fn drop(&mut self) {
        if self.path.symlink_metadata().is_ok_and(|metadata| {
            metadata.file_type().is_socket() && (metadata.dev(), metadata.ino()) == self.identity
        }) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn lock(path: &Path) -> io::Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "daemon lock must be a private regular file owned by this user",
        ));
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "another daemon holds this socket or catalogue lock",
        ));
    }
    Ok(file)
}

fn remove_stale(path: &Path) -> io::Result<()> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.permissions().mode() & 0o077 != 0
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refuse to replace a non-socket, symlink, foreign or accessible socket",
        ));
    }
    let probe = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    probe.set_nonblocking(true)?;
    match probe.connect(&SockAddr::unix(path)?) {
        Err(error) if error.kind() == io::ErrorKind::ConnectionRefused => {
            std::fs::remove_file(path)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            "socket already has a live listener or cannot be proved stale",
        )),
    }
}

#[cfg(test)]
mod tests;
