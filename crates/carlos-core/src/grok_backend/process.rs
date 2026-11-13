//! Grok ACP stdio process: spawn, JSON-RPC IO, and stream routing.

#[path = "process_client.rs"]
mod process_client;
#[path = "process_io.rs"]
mod process_io;
#[path = "process_writer.rs"]
mod process_writer;

use std::collections::{HashMap, HashSet};
use std::env;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicU64;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use self::process_client::respond_to_agent_request;
use self::process_writer::InputWriter;
use super::translate::{finish_prompt_turn, translate_grok_notification};
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
    events_tx: mpsc::Sender<String>,
    translation: Arc<Mutex<GrokTranslationState>>,
) -> Result<GrokProcess> {
    start_command(build_grok_command(cwd, runtime), events_tx, translation)
}

/// Start the chosen process through the same initialized native ACP reader.
pub(super) fn start_command(
    mut command: Command,
    events_tx: mpsc::Sender<String>,
    translation: Arc<Mutex<GrokTranslationState>>,
) -> Result<GrokProcess> {
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
        events_tx,
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
        }),
        reader_thread: Some(reader_thread),
        stderr_thread: Some(stderr_thread),
    };
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

#[cfg(any(test, feature = "test-support"))]
/// Build the exact Grok launch command without starting a process.
pub fn build_grok_command_for_test(cwd: &Path, runtime: &GrokRuntimeSettings) -> Command {
    build_grok_command(cwd, runtime)
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
        self.stdin.close();
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

// --- Reader ---

fn spawn_reader_thread(
    stdout: std::process::ChildStdout,
    events_tx: mpsc::Sender<String>,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    prompt_in_flight: Arc<Mutex<HashSet<u64>>>,
    stdin: Arc<InputWriter>,
    translation: Arc<Mutex<GrokTranslationState>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            let n = match reader.read_line(&mut line) {
                Ok(n) => n,
                Err(_) => break,
            };
            if n == 0 {
                break;
            }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            route_line(
                trimmed,
                &events_tx,
                &pending,
                &prompt_in_flight,
                &stdin,
                &translation,
            );
        }
        stdin.close();
        if let Ok(mut requests) = pending.lock() {
            for (id, reply) in requests.drain() {
                let _ = reply.send(
                    json!({"id":id,"error":{"message":"Grok stdout closed before replying"}})
                        .to_string(),
                );
            }
        }
    })
}

fn spawn_stderr_thread(
    stderr: std::process::ChildStderr,
    captured: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Ok(mut captured) = captured.lock() {
                        captured.push_str(&line);
                    }
                }
            }
        }
    })
}

fn route_line(
    line: &str,
    events_tx: &mpsc::Sender<String>,
    pending: &Arc<Mutex<HashMap<u64, mpsc::Sender<String>>>>,
    prompt_in_flight: &Arc<Mutex<HashSet<u64>>>,
    stdin: &Arc<InputWriter>,
    translation: &Arc<Mutex<GrokTranslationState>>,
) {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if let Some(id) = parsed.get("id").and_then(Value::as_u64) {
        if parsed.get("method").and_then(Value::as_str).is_some() {
            respond_to_agent_request(&parsed, stdin);
            return;
        }
        if let Some(last) = take_prompt_id(prompt_in_flight, id) {
            emit_prompt_finished(&parsed, events_tx, translation, last);
            return;
        }
        if let Ok(mut pending) = pending.lock() {
            if let Some(tx) = pending.remove(&id) {
                let _ = tx.send(line.to_string());
            }
        }
        return;
    }
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return;
    };
    let params = parsed.get("params").cloned().unwrap_or(Value::Null);
    let Ok(mut state) = translation.lock() else {
        return;
    };
    for synthetic in translate_grok_notification(&mut state, method, &params).lines {
        let _ = events_tx.send(synthetic);
    }
}

fn take_prompt_id(prompt_in_flight: &Arc<Mutex<HashSet<u64>>>, id: u64) -> Option<bool> {
    let Ok(mut prompts) = prompt_in_flight.lock() else {
        return None;
    };
    if !prompts.remove(&id) {
        return None;
    }
    Some(prompts.is_empty())
}

fn emit_prompt_finished(
    parsed: &Value,
    events_tx: &mpsc::Sender<String>,
    translation: &Arc<Mutex<GrokTranslationState>>,
    last: bool,
) {
    let interrupted = parsed
        .get("result")
        .and_then(|result| result.get("stopReason"))
        .and_then(Value::as_str)
        == Some("cancelled")
        || parsed.get("error").is_some();
    if let Some(message) = parsed
        .get("error")
        .and_then(|error| error.get("message"))
        .and_then(Value::as_str)
    {
        let _ = events_tx.send(
            json!({
                "method": "error",
                "params": { "message": format!("Grok prompt failed: {message}") },
            })
            .to_string(),
        );
    }
    if !last {
        return;
    }
    let Ok(mut state) = translation.lock() else {
        return;
    };
    for synthetic in finish_prompt_turn(&mut state, interrupted).lines {
        let _ = events_tx.send(synthetic);
    }
}

impl Drop for GrokProcess {
    fn drop(&mut self) {
        self.stop();
    }
}
