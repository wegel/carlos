//! Start the headless local daemon with explicit private socket and catalogue paths.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use carlosd::server::{run, Config};
use signal_hook::consts::{SIGINT, SIGTERM};

fn main() -> Result<()> {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == "--build-info")
    {
        println!("{}", serde_json::to_string(&carlosd::build_info())?);
        return Ok(());
    }
    if carlos_core::summaries::reader::maybe_run()? {
        return Ok(());
    }
    let Some(config) = arguments()? else {
        return Ok(());
    };
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, stop.clone()).context("register interrupt signal")?;
    signal_hook::flag::register(SIGTERM, stop.clone()).context("register terminate signal")?;
    run(config, stop).context("run carlosd")
}

fn arguments() -> Result<Option<Config>> {
    let mut arguments = std::env::args_os().skip(1);
    let mut socket = None;
    let mut state = None;
    let mut codex_socket = None;
    while let Some(option) = arguments.next() {
        if option == "--help" || option == "-h" {
            println!("carlosd --socket PATH --state-dir PATH [--codex-socket PATH]");
            return Ok(None);
        }
        let target = match option.to_str() {
            Some("--socket") => &mut socket,
            Some("--state-dir") => &mut state,
            Some("--codex-socket") => &mut codex_socket,
            _ => bail!("unknown option: {}", option.to_string_lossy()),
        };
        if target.is_some() {
            bail!("duplicate option: {}", option.to_string_lossy());
        }
        *target = Some(PathBuf::from(
            arguments.next().context("option requires a path")?,
        ));
    }
    Ok(Some(Config {
        socket: socket.context("--socket is required")?,
        state: state.context("--state-dir is required")?,
        codex_socket,
    }))
}
