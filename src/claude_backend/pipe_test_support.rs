//! Keeps fixture control sockets open across exec, including dup2's same-descriptor case.

use std::io;
use std::os::fd::RawFd;

/// Duplicate a fixture socket and explicitly clear close-on-exec on the destination.
/// The caller must run this only in the child immediately before exec.
pub(super) unsafe fn inherit_descriptor(source: RawFd, target: RawFd) -> io::Result<()> {
    if libc::dup2(source, target) == -1 {
        return Err(io::Error::last_os_error());
    }
    let flags = libc::fcntl(target, libc::F_GETFD);
    if flags == -1 || libc::fcntl(target, libc::F_SETFD, flags & !libc::FD_CLOEXEC) == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
