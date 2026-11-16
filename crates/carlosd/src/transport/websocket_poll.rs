//! Sleep until TCP, queued output or a fixed handshake/write deadline can make progress.

use std::io::{self, Read};
use std::net::TcpStream;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::time::Instant;

pub(super) fn wait(
    stream: &TcpStream,
    mut wake: &UnixStream,
    reading: bool,
    writing: bool,
    deadline: Option<Instant>,
) -> io::Result<()> {
    let events = if reading { libc::POLLIN } else { 0 } | if writing { libc::POLLOUT } else { 0 };
    let mut fds = [
        libc::pollfd {
            fd: stream.as_raw_fd(),
            events,
            revents: 0,
        },
        libc::pollfd {
            fd: wake.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];
    let timeout = deadline.map_or(-1, |deadline| {
        deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .saturating_add(1)
            .min(i32::MAX as u128) as i32
    });
    if unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, timeout) } < 0 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
            return Err(error);
        }
        return Ok(());
    }
    if fds[0].revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
        return Err(io::ErrorKind::ConnectionAborted.into());
    }
    if fds[1].revents != 0 {
        let mut bytes = [0; 256];
        loop {
            match wake.read(&mut bytes) {
                Ok(0) => return Err(io::ErrorKind::BrokenPipe.into()),
                Ok(_) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}
