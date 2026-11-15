//! Grok ACP stdio process: spawn, JSON-RPC IO, and stream routing.

#[path = "process_client.rs"]
mod process_client;
#[path = "process_io.rs"]
mod process_io;
#[path = "process_reader.rs"]
mod process_reader;
#[path = "process_writer.rs"]
mod process_writer;
use self::process_reader::{spawn_reader_thread, spawn_stderr_thread};
use crate::launch_cancel::LaunchCancellation;
use crate::protocol_events::EventQueue;

use std::collections::{HashMap, HashSet};
use std::env;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use self::process_writer::InputWriter;
use super::types::{GrokRuntimeSettings, GrokTranslationState};

const INITIALIZE_TIMEOUT: Duration = Duration::from_secs(20);

// --- Process ---

pub(super) struct GrokProcess {
    child: Arc<Mutex<Child>>,
    io: Arc<GrokIo>,
    reader_thread: Option<thread::JoinHandle<()>>,
    stderr_thread: Option<thread::JoinHandle<()>>,
}

/// Share requests independently from the child and its event consumer.
pub(super) struct GrokIo {
    stdin: Arc<InputWriter>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    prompt_in_flight: Arc<Mutex<HashSet<u64>>>,
    next_id: AtomicU64,
    events: EventQueue,
}

impl std::ops::Deref for GrokProcess {
    type Target = GrokIo;
    fn deref(&self) -> &Self::Target {
        &self.io
    }
}

impl GrokProcess {
    pub(super) fn commands(&self) -> Arc<GrokIo> {
        self.io.clone()
    }
}

// --- Launch ---

pub(super) fn start_grok_process(
    cwd: &Path,
    runtime: &GrokRuntimeSettings,
    events_tx: EventQueue,
    translation: Arc<Mutex<GrokTranslationState>>,
    cancel: &LaunchCancellation,
) -> Result<GrokProcess> {
    start_with_events(
        build_grok_command(cwd, runtime),
        events_tx,
        translation,
        cancel,
    )
}

pub(super) fn start_with_events(
    mut command: Command,
    events_tx: EventQueue,
    translation: Arc<Mutex<GrokTranslationState>>,
    cancel: &LaunchCancellation,
) -> Result<GrokProcess> {
    cancel.check()?;
    let mut child = command
        .spawn()
        .context("failed to spawn `grok agent stdio`")?;
    let stdin = child.stdin.take().context("missing Grok stdin")?;
    let stdout = child.stdout.take().context("missing Grok stdout")?;
    let stderr = child.stderr.take().context("missing Grok stderr")?;
    let pending = Arc::new(Mutex::new(HashMap::new()));
    let prompt_in_flight = Arc::new(Mutex::new(HashSet::new()));
    let child = Arc::new(Mutex::new(child));
    let stdin = Arc::new(InputWriter::start(stdin, child.clone()));
    let captured_stderr = Arc::new(Mutex::new(String::new()));
    let reader_thread = spawn_reader_thread(
        stdout,
        events_tx.clone(),
        Arc::clone(&pending),
        Arc::clone(&prompt_in_flight),
        Arc::clone(&stdin),
        Arc::clone(&translation),
    );
    let stderr_thread = spawn_stderr_thread(stderr, Arc::clone(&captured_stderr));
    let process = GrokProcess {
        child,
        io: Arc::new(GrokIo {
            stdin,
            pending,
            prompt_in_flight,
            next_id: AtomicU64::new(1),
            events: events_tx,
        }),
        reader_thread: Some(reader_thread),
        stderr_thread: Some(stderr_thread),
    };
    let io = process.commands();
    cancel.register(move || io.cancel())?;
    if let Err(err) = initialize_agent(&process) {
        let stderr = captured_stderr
            .lock()
            .ok()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty());
        if let Some(stderr) = stderr {
            bail!("{err:#}; grok stderr: {stderr}");
        }
        return Err(err);
    }
    cancel.check()?;
    Ok(process)
}

fn build_grok_command(cwd: &Path, runtime: &GrokRuntimeSettings) -> Command {
    let mut command = Command::new("grok");
    command.args(["agent", "--always-approve", "--no-leader", "stdio"]);
    if let Some(model) = runtime.model.as_deref() {
        command.args(["--model", model]);
    }
    if let Some(effort) = runtime.effort.as_deref() {
        command.args(["--reasoning-effort", effort]);
    }
    command
        .env("GROK_DISABLE_AUTOUPDATER", "1")
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(home) = env::var_os("GROK_HOME") {
        command.env("GROK_HOME", home);
    }
    command
}

fn initialize_agent(process: &GrokProcess) -> Result<()> {
    let response = process.request(
        "initialize",
        json!({
            "protocolVersion": 1,
            "clientCapabilities": {},
            "clientInfo": {
                "name": "carlos",
                "title": "Carlos",
                "version": "0.1.0",
            }
        }),
        INITIALIZE_TIMEOUT,
    )?;
    let parsed: Value = serde_json::from_str(&response).context("invalid Grok initialize JSON")?;
    if parsed.get("error").is_some() {
        bail!(
            "Grok initialize failed: {}",
            parsed
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("unknown error")
        );
    }
    Ok(())
}

// --- RPC ---

impl GrokProcess {
    pub(super) fn stop(&mut self) {
        self.io.cancel();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            let state = self.child.lock().expect("Grok child").try_wait();
            match state {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }
        let _ = self.reader_thread.take();
        let _ = self.stderr_thread.take();
    }
}

impl Drop for GrokProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
#[path = "launch_command_tests.rs"]
mod tests;
