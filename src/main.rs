mod app;
mod backend;
mod clipboard;
mod claude_backend;
mod event;
mod protocol;
mod theme;

fn main() {
    if let Err(err) = app::run() {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}
