//! Keep native session owners alive independently of terminal, phone and browser sockets.

pub mod limits;
pub mod registry;
pub mod retry;

#[cfg(unix)]
mod transport;

#[cfg(unix)]
mod acceptor;
#[cfg(unix)]
mod manager;
#[cfg(unix)]
mod network;
#[cfg(unix)]
pub mod server;

#[cfg(unix)]
pub mod socket;

/// Report the daemon package's version and its shared source compatibility marker.
pub fn build_info() -> carlos_proto::identity::BuildInfo {
    carlos_proto::identity::BuildInfo::new(env!("CARGO_PKG_VERSION"), env!("CARLOS_SOURCE_ID"))
}
