//! Keep native session owners alive independently of terminal, phone and browser sockets.

pub mod limits;
pub mod registry;
pub mod retry;

#[cfg(unix)]
mod transport;

#[cfg(unix)]
mod manager;
#[cfg(unix)]
pub mod server;

#[cfg(unix)]
pub mod socket;
