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
mod protocol_params;
mod theme;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
