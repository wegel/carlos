//! Versioned client messages without sockets, threads, clocks or terminal types.

#[cfg(test)]
mod approval_tests;
pub mod bytes;
mod command_types;
pub mod commands;
pub mod context_defaults;
pub mod events;
pub mod goals;
pub mod ids;
pub mod output;
pub mod queue;
pub mod rpc;
pub mod session;
pub mod settings;
pub mod source;
pub mod startup;
pub mod status;
pub mod submissions;
pub mod task_activity;
pub mod task_facts;
pub mod task_overview;
pub mod task_windows;
pub mod tasks;

/// The only client protocol version supported by this release.
pub const PROTOCOL_VERSION: u32 = 1;
