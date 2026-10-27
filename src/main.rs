//! Carlos: a terminal frontend for Codex, Claude, and Grok.

mod app;
mod backend;
mod claude_backend;
mod clipboard;
#[cfg(feature = "dictation")]
mod dictation;
mod event;
mod grok_backend;
mod protocol;
mod protocol_history;
mod protocol_params;
mod protocol_rewind;
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
