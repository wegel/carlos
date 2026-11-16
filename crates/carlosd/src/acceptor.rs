//! Accept trusted Unix peers and bounded TCP upgrades without blocking the session manager.

use std::io;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::fd::AsRawFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::{self, JoinHandle};

use crate::network::{self, Accepted, Admissions};
use crate::socket::verify_peer;

pub(crate) struct Listeners {
    pub local: UnixListener,
    pub network: Option<TcpListener>,
    pub origin: Option<String>,
}

pub(crate) struct Channels {
    pub local: mpsc::SyncSender<UnixStream>,
    pub network: mpsc::SyncSender<Accepted>,
    pub wake: mpsc::SyncSender<()>,
}

struct Pending {
    socket: TcpStream,
    worker: JoinHandle<()>,
}

pub(crate) fn run(listeners: Listeners, channels: Channels, stop: Arc<AtomicBool>) {
    let pool = Arc::new(Admissions::default());
    let mut pending = Vec::new();
    while !stop.load(Ordering::SeqCst) {
        prune(&mut pending);
        let mut fds = [
            libc::pollfd {
                fd: listeners.local.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: listeners.network.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), 2, 100) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            stop.store(true, Ordering::SeqCst);
            let _ = channels.wake.try_send(());
            break;
        }
        if fds[0].revents != 0 {
            accept_local(&listeners.local, &channels, &stop);
        }
        if fds[1].revents != 0 {
            if let Some(listener) = &listeners.network {
                accept_network(
                    listener,
                    &listeners.origin,
                    &pool,
                    &channels,
                    &mut pending,
                    &stop,
                );
            }
        }
    }
    for pending in pending {
        let _ = pending.socket.shutdown(Shutdown::Both);
        let _ = pending.worker.join();
    }
}

fn accept_local(listener: &UnixListener, channels: &Channels, stop: &AtomicBool) {
    match listener.accept() {
        Ok((stream, _)) => {
            if verify_peer(&stream).is_ok() {
                let _ = channels.local.try_send(stream);
                let _ = channels.wake.try_send(());
            }
        }
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) => {}
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            let _ = channels.wake.try_send(());
        }
    }
}

fn accept_network(
    listener: &TcpListener,
    origin: &Option<String>,
    pool: &Arc<Admissions>,
    channels: &Channels,
    pending: &mut Vec<Pending>,
    stop: &AtomicBool,
) {
    let stream = match listener.accept() {
        Ok((stream, _)) => stream,
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
            ) =>
        {
            return
        }
        Err(_) => {
            stop.store(true, Ordering::SeqCst);
            let _ = channels.wake.try_send(());
            return;
        }
    };
    if pending.len() >= crate::limits::NETWORK_UNAUTHENTICATED {
        return;
    }
    let Some(admission) = pool.admit() else {
        return;
    };
    let Ok(control) = stream.try_clone() else {
        return;
    };
    let opened = channels.network.clone();
    let wake = channels.wake.clone();
    let origin = origin.clone();
    let worker = thread::spawn(move || {
        if let Ok(socket) = network::upgrade(stream, origin, admission.deadline) {
            let _ = opened.try_send(Accepted { socket, admission });
            let _ = wake.try_send(());
        }
    });
    pending.push(Pending {
        socket: control,
        worker,
    });
}

fn prune(pending: &mut Vec<Pending>) {
    let mut index = 0;
    while index < pending.len() {
        if pending[index].worker.is_finished() {
            let _ = pending.swap_remove(index).worker.join();
        } else {
            index += 1;
        }
    }
}
