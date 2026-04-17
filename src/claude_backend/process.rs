//! Claude CLI process launch, startup probing, and stream forwarding.

// --- Imports ---

use std::env;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use serde_json::json;

use super::translate::translate_claude_line;
use super::types::{
    ClaudeLaunchMode, ClaudeRuntimeSettings, ClaudeTranslationState,
};

const CLAUDE_PLAN_MODE_BLOCKLIST: [&str; 2] = ["EnterPlanMode", "Agent(Plan)"];
const CLAUDE_STARTUP_TIMEOUT: Duration = Duration::from_secs(2);
static CLAUDE_STREAM_INSTANCE_SEQ: AtomicU64 = AtomicU64::new(1);

// --- Process types ---

pub(super) struct ClaudeProcess {
    pub(super) child: Child,
    pub(super) stdin: ChildStdin,
    pub(super) reader_thread: Option<thread::JoinHandle<()>>,
    pub(super) stderr_thread: Option<thread::JoinHandle<()>>,
}

enum ClaudeStartupEvent {
    StreamReady,
    StreamClosed,
}

// --- Command building ---

fn claude_allow_plan_mode() -> bool {
    env::var("CARLOS_CLAUDE_ALLOW_PLAN_MODE")
        .ok()
        .map(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return true;
            }
            !matches!(
                trimmed.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

fn next_claude_stream_instance_seq() -> u64 {
    CLAUDE_STREAM_INSTANCE_SEQ.fetch_add(1, Ordering::Relaxed)
}

fn build_claude_command(
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
    runtime_settings: &ClaudeRuntimeSettings,
) -> Command {
    let mut command = Command::new("claude");
    command.args([
        "-p",
        "--input-format", "stream-json",
        "--output-format", "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--permission-mode", "bypassPermissions",
    ]);
    if !claude_allow_plan_mode() {
        command.arg("--disallowedTools");
        for tool in CLAUDE_PLAN_MODE_BLOCKLIST {
            command.arg(tool);
        }
    }
    if let Some(config_dir) = env::var_os("CLAUDE_CONFIG_DIR") {
        command.env("CLAUDE_CONFIG_DIR", config_dir);
    }
    command.current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    match launch_mode {
        ClaudeLaunchMode::New => {}
        ClaudeLaunchMode::Resume(session_id) => {
            command.arg("--resume").arg(session_id);
        }
        ClaudeLaunchMode::Continue => {
            command.arg("--continue");
        }
    }
    if let Some(model) = runtime_settings.model.as_deref() {
        command.arg("--model").arg(model);
    }
    if let Some(effort) = runtime_settings.effort.as_deref() {
        command.arg("--effort").arg(effort);
    }
    command
}

#[cfg(test)]
pub(crate) fn build_claude_command_for_test(
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
) -> Command {
    build_claude_command(cwd, launch_mode, &ClaudeRuntimeSettings::default())
}

// --- Startup probing ---

#[cfg(test)]
pub(crate) fn probe_claude_startup_for_test(command: &mut Command) -> Result<()> {
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().context("failed to spawn startup probe")?;
    let stdout = child.stdout.take().context("missing probe stdout")?;
    let stderr = child.stderr.take().context("missing probe stderr")?;
    let (startup_tx, startup_rx) = mpsc::channel();
    let captured_stderr = Arc::new(Mutex::new(String::new()));
    let reader_thread = spawn_reader_thread(
        stdout,
        mpsc::channel::<String>().0,
        Arc::new(Mutex::new(None)),
        Some(startup_tx),
    );
    let stderr_thread = spawn_stderr_thread(stderr, Arc::clone(&captured_stderr));
    let result = await_claude_startup(&mut child, &startup_rx, &captured_stderr);
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader_thread.join();
    let _ = stderr_thread.join();
    result
}

fn await_claude_startup(
    child: &mut Child,
    startup_rx: &mpsc::Receiver<ClaudeStartupEvent>,
    captured_stderr: &Arc<Mutex<String>>,
) -> Result<()> {
    let deadline = Instant::now() + CLAUDE_STARTUP_TIMEOUT;
    loop {
        match startup_rx.try_recv() {
            Ok(ClaudeStartupEvent::StreamReady) => return Ok(()),
            Ok(ClaudeStartupEvent::StreamClosed) => {
                if let Some(status) = child.try_wait()? {
                    bail!(format_claude_startup_error(status, captured_stderr));
                }
                return Ok(());
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                if let Some(status) = child.try_wait()? {
                    bail!(format_claude_startup_error(status, captured_stderr));
                }
                return Ok(());
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if let Some(status) = child.try_wait()? {
            thread::sleep(Duration::from_millis(10));
            bail!(format_claude_startup_error(status, captured_stderr));
        }
        if Instant::now() >= deadline {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn format_claude_startup_error(
    status: ExitStatus,
    captured_stderr: &Arc<Mutex<String>>,
) -> String {
    let stderr = captured_stderr
        .lock()
        .ok()
        .map(|captured| captured.trim().to_string())
        .filter(|captured| !captured.is_empty());
    match stderr {
        Some(stderr) => format!("`claude` exited during startup ({status}): {stderr}"),
        None => format!("`claude` exited during startup ({status})"),
    }
}

// --- Live stream forwarding ---

fn spawn_reader_thread(
    stdout: std::process::ChildStdout,
    events_tx: mpsc::Sender<String>,
    session_id: Arc<Mutex<Option<String>>>,
    startup_tx: Option<mpsc::Sender<ClaudeStartupEvent>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        let mut state = ClaudeTranslationState {
            stream_instance_seq: next_claude_stream_instance_seq(),
            ..ClaudeTranslationState::default()
        };
        let mut startup_tx = startup_tx;
        loop {
            line.clear();
            match std::io::BufRead::read_line(&mut reader, &mut line) {
                Ok(0) | Err(_) => {
                    if let Some(tx) = startup_tx.take() {
                        let _ = tx.send(ClaudeStartupEvent::StreamClosed);
                    }
                    break;
                }
                _ => {}
            }
            if let Some(tx) = startup_tx.take() {
                let _ = tx.send(ClaudeStartupEvent::StreamReady);
            }
            let trimmed = line.trim_end_matches(['\n', '\r']);
            if trimmed.is_empty() {
                continue;
            }
            let Ok(translated) = translate_claude_line(&mut state, trimmed) else {
                continue;
            };
            if let Some(sid) = state
                .session_id
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                if let Ok(mut cur) = session_id.lock() {
                    if cur.as_deref() != Some(sid) {
                        *cur = Some(sid.to_string());
                    }
                }
            }
            for synthetic in translated.lines {
                let _ = events_tx.send(synthetic);
            }
        }
    })
}

fn spawn_stderr_thread(
    stderr: ChildStderr,
    captured_stderr: Arc<Mutex<String>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut reader = std::io::BufReader::new(stderr);
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    if let Ok(text) = std::str::from_utf8(&buf[..count]) {
                        if let Ok(mut captured) = captured_stderr.lock() {
                            captured.push_str(text);
                        }
                    } else {
                        let text = String::from_utf8_lossy(&buf[..count]);
                        if let Ok(mut captured) = captured_stderr.lock() {
                            captured.push_str(&text);
                        }
                    }
                }
            }
        }
    })
}

#[cfg(test)]
pub(crate) fn collect_live_forwarded_lines_for_test(lines: &[&str]) -> Vec<String> {
    let mut state = ClaudeTranslationState {
        stream_instance_seq: next_claude_stream_instance_seq(),
        ..ClaudeTranslationState::default()
    };
    let mut out = Vec::new();
    for line in lines {
        let translated = translate_claude_line(&mut state, line).expect("translate");
        out.extend(translated.lines);
    }
    out
}

// --- Public API ---

pub(super) fn start_claude_process(
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
    runtime_settings: &ClaudeRuntimeSettings,
    events_tx: mpsc::Sender<String>,
    current_session_id: Arc<Mutex<Option<String>>>,
) -> Result<ClaudeProcess> {
    let mut command = build_claude_command(cwd, launch_mode, runtime_settings);
    let mut child = command.spawn().context("failed to spawn `claude`")?;
    let stdin = child.stdin.take().context("missing child stdin")?;
    let stdout = child.stdout.take().context("missing child stdout")?;
    let stderr = child.stderr.take().context("missing child stderr")?;
    let (startup_tx, startup_rx) = mpsc::channel();
    let captured_stderr = Arc::new(Mutex::new(String::new()));
    let reader_thread = spawn_reader_thread(
        stdout,
        events_tx,
        current_session_id,
        Some(startup_tx),
    );
    let stderr_thread = spawn_stderr_thread(stderr, Arc::clone(&captured_stderr));
    if let Err(err) = await_claude_startup(&mut child, &startup_rx, &captured_stderr) {
        let _ = child.kill();
        let _ = child.wait();
        let _ = reader_thread.join();
        let _ = stderr_thread.join();
        return Err(err);
    }

    Ok(ClaudeProcess {
        child,
        stdin,
        reader_thread: Some(reader_thread),
        stderr_thread: Some(stderr_thread),
    })
}

impl ClaudeProcess {
    pub(super) fn stop(&mut self) {
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_millis(250);
        loop {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                _ => break,
            }
        }
        let _ = self.reader_thread.take();
        let _ = self.stderr_thread.take();
    }

    pub(super) fn write_user_message(&mut self, text: &str) -> Result<String> {
        let line = json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": [{
                    "type": "text",
                    "text": text,
                }]
            }
        })
        .to_string();

        self.stdin.write_all(line.as_bytes())?;
        self.stdin.write_all(b"\n")?;
        self.stdin.flush()?;

        Ok(json!({
            "jsonrpc": "2.0",
            "result": {}
        })
        .to_string())
    }
}
