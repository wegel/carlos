//! Session facts and backend commands that run without a terminal client.

pub mod backend;
pub mod backend_commands;
pub mod claude_backend;
pub mod codex_connection;
mod codex_diagnostics;
pub mod codex_snapshot;
mod codex_snapshot_reads;
pub mod context_preferences;
pub mod dispatch_guard;
pub mod grok_backend;
pub mod launch_cancel;
pub mod native_form;
pub mod perf_fixture;
pub mod protocol;
mod protocol_capabilities;
mod protocol_events;
mod protocol_forms;
pub mod protocol_history;
pub mod protocol_params;
mod protocol_reader;
mod protocol_rewind;
mod protocol_rewind_guard;
mod protocol_socket;
mod protocol_state;
mod protocol_writer;
pub mod session;
pub use session::engine;
pub mod summaries;

#[cfg(test)]
#[path = "tests/backend_owner_codex_tests.rs"]
mod backend_owner_codex_tests;
#[cfg(test)]
#[path = "tests/codex_control_guard_tests.rs"]
mod codex_control_guard_tests;

#[cfg(test)]
#[path = "tests/writer_guard_test_support.rs"]
mod writer_guard_test_support;
