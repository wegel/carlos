//! Bound connection and greeting waits before a managed terminal submits any session work.

use std::io;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use carlos_client::codec::Incoming;
use carlos_client::Client;
use carlos_proto::commands::HelloResult;
use socket2::{Domain, SockAddr, Socket, Type};

use crate::app::remote::Transport;

pub(super) fn connect(
    path: &Path,
    endpoint: Option<&Path>,
    deadline: Instant,
) -> Result<Option<(Client, Transport)>> {
    match path.symlink_metadata() {
        Ok(metadata) => {
            if !metadata.file_type().is_socket() || metadata.uid() != unsafe { libc::geteuid() } {
                bail!(
                    "managed daemon path must be a socket owned by this user, not a symlink: {}",
                    path.display()
                );
            }
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let Some(stream) = open(path, deadline).context("connect to managed Carlos daemon")? else {
        return Ok(None);
    };
    let mut client = Client::default();
    let mut transport = Transport::from_stream(stream, &mut client)?;
    loop {
        transport.flush()?;
        for message in transport.receive()? {
            if let Incoming::Success { result, .. } = &message {
                validate(
                    &serde_json::from_value::<HelloResult>(result.clone())?,
                    endpoint,
                    crate::app::cli::env_flag_enabled("CARLOS_STRICT_DAEMON_BUILD"),
                )?;
            }
            // This fresh client only greets; it has no session reads or watches to drive.
            let _ = client.receive(transport.connection, message)?;
            if client.is_ready(transport.connection) {
                // The private parent protects bind's brief interval before the daemon chmods.
                if path.symlink_metadata()?.permissions().mode() & 0o077 != 0 {
                    bail!("managed daemon socket must be private: {}", path.display());
                }
                return Ok(Some((client, transport)));
            }
        }
        if Instant::now() >= deadline {
            bail!("Carlos daemon did not greet this terminal before the startup deadline");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

pub(super) fn validate(hello: &HelloResult, endpoint: Option<&Path>, strict: bool) -> Result<()> {
    if hello.protocol != carlos_proto::PROTOCOL_VERSION {
        bail!(
            "running Carlos daemon protocol {} is incompatible with terminal protocol {}",
            hello.protocol,
            carlos_proto::PROTOCOL_VERSION
        );
    }
    let Some(identity) = hello.identity.as_ref() else {
        if strict || endpoint.is_some() {
            bail!("managed daemon did not report the identity needed for the requested build or native endpoint check");
        }
        return Ok(());
    };
    if identity.build.protocol != hello.protocol {
        bail!(
            "running Carlos daemon reported inconsistent protocol versions (daemon PID {})",
            identity.process
        );
    }
    if strict && identity.build != crate::build_info() {
        bail!("running Carlos daemon build differs from this terminal (daemon PID {}); CARLOS_STRICT_DAEMON_BUILD requires an exact build match", identity.process);
    }
    if let Some(endpoint) = endpoint {
        if identity.codex_socket.as_deref() != Some(endpoint.as_os_str().as_bytes()) {
            bail!("running Carlos daemon uses a different native Codex endpoint; --codex-socket cannot change a live daemon");
        }
    }
    Ok(())
}

fn open(path: &Path, deadline: Instant) -> io::Result<Option<UnixStream>> {
    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
    socket.set_nonblocking(true)?;
    match socket.connect(&SockAddr::unix(path)?) {
        Ok(()) => return Ok(Some(UnixStream::from(OwnedFd::from(socket)))),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            return Ok(None)
        }
        Err(error)
            if matches!(
                error.raw_os_error(),
                Some(libc::EINPROGRESS) | Some(libc::EAGAIN)
            ) => {}
        Err(error) => return Err(error),
    }
    loop {
        if let Some(error) = socket.take_error()? {
            return Err(error);
        }
        if socket.peer_addr().is_ok() {
            return Ok(Some(UnixStream::from(OwnedFd::from(socket))));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::ErrorKind::TimedOut.into());
        }
        let mut fd = libc::pollfd {
            fd: socket.as_raw_fd(),
            events: libc::POLLOUT,
            revents: 0,
        };
        if unsafe { libc::poll(&mut fd, 1, remaining.as_millis().clamp(1, 20) as i32) } < 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}
