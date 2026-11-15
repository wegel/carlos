//! Carlos: a terminal frontend for Codex, Claude, and Grok.

mod app;
mod clipboard;
#[cfg(feature = "dictation")]
mod dictation;
mod event;
mod theme;

fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--build-info")
    {
        println!(
            "{}",
            serde_json::to_string(&build_info()).expect("build identity JSON")
        );
        return;
    }
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

fn build_info() -> carlos_proto::identity::BuildInfo {
    carlos_proto::identity::BuildInfo::new(env!("CARGO_PKG_VERSION"), env!("CARLOS_SOURCE_ID"))
}
