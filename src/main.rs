//! Carlos: a terminal frontend for Codex, Claude, and Grok.

mod app;
mod backend;
mod claude_backend;
mod clipboard;
mod codex_connection;
mod codex_diagnostics;
mod codex_snapshot;
mod codex_snapshot_reads;
#[cfg(feature = "dictation")]
mod dictation;
mod event;
mod grok_backend;
mod native_form;
mod protocol;
mod protocol_capabilities;
mod protocol_events;
mod protocol_forms;
mod protocol_history;
mod protocol_params;
mod protocol_reader;
mod protocol_rewind;
mod protocol_rewind_guard;
mod protocol_socket;
mod protocol_state;
mod protocol_writer;
mod summaries;
mod theme;

fn main() {
    let result = summaries::reader::maybe_run()
        .and_then(|handled| if handled { Ok(()) } else { app::run() });
    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
