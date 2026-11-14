//! Keep native session owners alive independently of terminal, phone and browser sockets.

pub mod limits;
pub mod registry;
pub mod retry;

#[cfg(unix)]
pub mod socket;
