//! Versioned client messages without sockets, threads, clocks or terminal types.

pub mod bytes;
pub mod commands;
pub mod events;
pub mod ids;
pub mod output;
pub mod session;
pub mod source;
pub mod submissions;

/// The only client protocol version supported by this release.
pub const PROTOCOL_VERSION: u32 = 1;
