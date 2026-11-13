//! Carlos: a terminal frontend for Codex, Claude, and Grok.

mod app;
mod clipboard;
mod context_preferences;
#[cfg(feature = "dictation")]
mod dictation;
mod event;
mod theme;

fn main() {
    let result = carlos_core::summaries::reader::maybe_run().and_then(|handled| {
        if handled {
            Ok(())
        } else {
            app::run()
        }
    });
    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
