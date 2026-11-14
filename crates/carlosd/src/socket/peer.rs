//! Confirm peer ownership with kernel credentials as well as private socket permissions.

use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;

/// Reject a local client whose OS user differs from the daemon's user.
pub fn verify_peer(stream: &UnixStream) -> io::Result<()> {
    if peer_uid(stream)? != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Unix peer belongs to another OS user",
        ));
    }
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn peer_uid(stream: &UnixStream) -> io::Result<libc::uid_t> {
    let mut credentials = std::mem::MaybeUninit::<libc::ucred>::uninit();
    let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let result = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            credentials.as_mut_ptr().cast(),
            &mut length,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    if length as usize != std::mem::size_of::<libc::ucred>() {
        return Err(io::Error::other(
            "kernel returned malformed peer credentials",
        ));
    }
    Ok(unsafe { credentials.assume_init() }.uid)
}

#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn peer_uid(stream: &UnixStream) -> io::Result<libc::uid_t> {
    let (mut uid, mut gid) = (0, 0);
    if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(uid)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn peer_uid(_stream: &UnixStream) -> io::Result<libc::uid_t> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "kernel peer credentials are unsupported on this Unix target",
    ))
}
