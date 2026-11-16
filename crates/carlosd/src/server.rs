//! Serve private Unix and optional authenticated network endpoints and stop every owned session only on explicit daemon shutdown.

use std::fmt;
use std::io;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;

use crate::manager::Manager;
use crate::registry::RegistryError;
use crate::socket::BoundSocket;

/// Select private state paths and an optional explicit WebSocket address.
pub struct Config {
    pub socket: PathBuf,
    pub state: PathBuf,
    pub codex_socket: Option<PathBuf>,
    pub listen: Option<SocketAddr>,
    pub allowed_origin: Option<String>,
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
    let network =
        bind_network(config.listen, config.allowed_origin.as_deref()).map_err(ServerError::Io)?;
    let state_guard = bound.state_guard().map_err(ServerError::Io)?;
    let mut manager =
        Manager::new(config.state, codex_socket, state_guard).map_err(ServerError::Registry)?;
    let listener = bound.listener().try_clone().map_err(ServerError::Io)?;
    let (opened, accepted) = mpsc::sync_channel(16);
    let (network_opened, network_accepted) = mpsc::sync_channel(crate::limits::NETWORK_PEERS);
    let wake = manager.wake();
    let accept_stop = stop.clone();
    let listeners = crate::acceptor::Listeners {
        local: listener,
        network,
        origin: config.allowed_origin,
    };
    let channels = crate::acceptor::Channels {
        local: opened,
        network: network_opened,
        wake,
    };
    let acceptor = thread::spawn(move || crate::acceptor::run(listeners, channels, accept_stop));
    eprintln!("carlosd: listening on {}", bound.path().display());
    manager.run(accepted, network_accepted, stop.clone());
    stop.store(true, Ordering::SeqCst);
    let _ = acceptor.join();
    Ok(())
}

fn bind_network(
    address: Option<SocketAddr>,
    origin: Option<&str>,
) -> io::Result<Option<TcpListener>> {
    let Some(address) = address else {
        if origin.is_some() {
            return Err(io::Error::other("--allow-origin requires --listen"));
        }
        return Ok(None);
    };
    if address.ip().is_unspecified()
        || address.ip().is_multicast()
        || matches!(address.ip(), std::net::IpAddr::V4(ip) if ip.is_broadcast())
    {
        return Err(io::Error::other(
            "--listen requires one explicit unicast interface address",
        ));
    }
    if let Some(origin) = origin {
        if origin.is_empty()
            || origin.len() > 1024
            || tungstenite::http::HeaderValue::from_str(origin).is_err()
        {
            return Err(io::Error::other(
                "--allow-origin requires one bounded exact Origin header value",
            ));
        }
    }
    let listener = TcpListener::bind(address)?;
    listener.set_nonblocking(true)?;
    eprintln!(
        "carlosd: network listening on ws://{}",
        listener.local_addr()?
    );
    Ok(Some(listener))
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
