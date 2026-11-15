//! Versioned client messages without sockets, threads, clocks or terminal types.

pub mod bytes;
pub mod commands;
pub mod events;
pub mod goals;
pub mod ids;
pub mod output;
pub mod queue;
pub mod rpc;
pub mod session;
pub mod settings;
pub mod source;
pub mod submissions;
pub mod task_facts;
pub mod task_overview;
pub mod tasks;

/// The only client protocol version supported by this release.
pub const PROTOCOL_VERSION: u32 = 1;
