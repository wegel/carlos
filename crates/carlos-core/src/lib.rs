//! Session facts and backend commands that run without a terminal client.

pub mod backend;
pub mod claude_backend;
pub mod codex_connection;
mod codex_diagnostics;
pub mod codex_snapshot;
mod codex_snapshot_reads;
pub mod grok_backend;
pub mod native_form;
pub mod protocol;
mod protocol_capabilities;
mod protocol_events;
mod protocol_forms;
pub mod protocol_history;
pub mod protocol_params;
mod protocol_reader;
pub mod protocol_rewind;
#[cfg(not(feature = "test-support"))]
mod protocol_rewind_guard;
#[cfg(feature = "test-support")]
pub mod protocol_rewind_guard;
mod protocol_socket;
mod protocol_state;
mod protocol_writer;
pub mod session;
pub mod summaries;
