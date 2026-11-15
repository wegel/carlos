//! Serve a private local endpoint and stop every owned session only on explicit daemon shutdown.

use std::fmt;
use std::io;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::manager::Manager;
use crate::registry::RegistryError;
use crate::socket::{verify_peer, BoundSocket};

/// Select explicit private paths; this server has no network listener or native method tunnel.
pub struct Config {
    pub socket: PathBuf,
    pub state: PathBuf,
    pub codex_socket: Option<PathBuf>,
}

/// Startup refuses unsafe paths, occupied sockets and invalid catalogues.
#[derive(Debug)]
pub enum ServerError {
    Io(io::Error),
    Registry(RegistryError),
}

/// Run until an explicit signal while device disconnects leave native session owners alive.
pub fn run(config: Config, stop: Arc<AtomicBool>) -> Result<(), ServerError> {
    let codex_socket = config
        .codex_socket
        .map(absolute)
        .transpose()
        .map_err(ServerError::Io)?;
    let bound = BoundSocket::bind(&config.socket, &config.state).map_err(ServerError::Io)?;
    let state_guard = bound.state_guard().map_err(ServerError::Io)?;
    let mut manager =
        Manager::new(config.state, codex_socket, state_guard).map_err(ServerError::Registry)?;
    let listener = bound.listener().try_clone().map_err(ServerError::Io)?;
    let (opened, accepted) = mpsc::sync_channel(16);
    let wake = manager.wake();
    let accept_stop = stop.clone();
    let acceptor = thread::spawn(move || {
        while !accept_stop.load(Ordering::SeqCst) {
            let mut fd = libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut fd, 1, 100) };
            if ready < 0 {
                if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                accept_stop.store(true, Ordering::SeqCst);
                let _ = wake.try_send(());
                break;
            }
            if ready == 0 {
                continue;
            }
            match listener.accept() {
                Ok((stream, _)) => {
                    if verify_peer(&stream).is_ok() {
                        let _ = opened.try_send(stream);
                        let _ = wake.try_send(());
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                Err(_) => {
                    accept_stop.store(true, Ordering::SeqCst);
                    let _ = wake.try_send(());
                    break;
                }
            }
        }
    });
    eprintln!("carlosd: listening on {}", bound.path().display());
    manager.run(accepted, stop.clone());
    stop.store(true, Ordering::SeqCst);
    let _ = acceptor.join();
    Ok(())
}

fn absolute(path: PathBuf) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    Ok(absolute.canonicalize().unwrap_or(absolute))
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => error.fmt(formatter),
            Self::Registry(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ServerError {}
