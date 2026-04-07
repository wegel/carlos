//! Carlos: a terminal frontend for codex app-server and the claude CLI.

mod app;
mod backend;
mod claude_backend;
mod clipboard;
mod event;
mod protocol;
mod protocol_params;
mod theme;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
