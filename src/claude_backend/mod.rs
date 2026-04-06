//! Claude CLI backend: process management, client lifecycle, and BackendClient trait impl.

mod exit_plan;
mod history;
mod snapshot;
mod translate;
mod types;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind};

pub(crate) use self::exit_plan::claude_approval_follow_up_text;
pub(crate) use self::history::{
    load_claude_local_history, load_claude_local_history_from_projects_root,
};
pub(crate) use self::translate::translate_claude_line;
pub(crate) use self::types::{
    claude_model_catalog, claude_project_dir_name, claude_recovery_launch_mode, ClaudeLaunchMode,
    ClaudeTranslationState, CLAUDE_PENDING_THREAD_ID,
};

use self::types::{normalize_runtime_arg, ClaudeRuntimeSettings};

// --- Client types ---

pub(crate) struct ClaudeClient {
    cwd: PathBuf,
    launch_mode: ClaudeLaunchMode,
    current_session_id: Arc<Mutex<Option<String>>>,
    runtime_settings: Mutex<ClaudeRuntimeSettings>,
    process: Mutex<ClaudeProcess>,
    events_tx: mpsc::Sender<String>,
    events_rx: Option<mpsc::Receiver<String>>,
}

struct ClaudeProcess {
    child: Child,
    stdin: ChildStdin,
    reader_thread: Option<thread::JoinHandle<()>>,
}

// --- Client construction and lifecycle ---

impl ClaudeClient {
    pub(crate) fn start(cwd: &Path, launch_mode: ClaudeLaunchMode) -> Result<Self> {
        let (events_tx, events_rx) = mpsc::channel::<String>();
        let current_session_id = Arc::new(Mutex::new(match &launch_mode {
            ClaudeLaunchMode::Resume(id) => Some(id.clone()),
            _ => None,
        }));
        let runtime_settings = ClaudeRuntimeSettings::default();
        let process = Self::spawn_process(
            cwd,
            &launch_mode,
            &runtime_settings,
            events_tx.clone(),
            Arc::clone(&current_session_id),
        )?;

        Ok(Self {
            cwd: cwd.to_path_buf(),
            launch_mode,
            current_session_id,
            runtime_settings: Mutex::new(runtime_settings),
            process: Mutex::new(process),
            events_tx,
            events_rx: Some(events_rx),
        })
    }

    fn spawn_process(
        cwd: &Path,
        launch_mode: &ClaudeLaunchMode,
        runtime_settings: &ClaudeRuntimeSettings,
        events_tx: mpsc::Sender<String>,
        current_session_id: Arc<Mutex<Option<String>>>,
    ) -> Result<ClaudeProcess> {
        let mut command = Command::new("claude");
        command.args([
            "-p",
            "--input-format", "stream-json",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-mode", "bypassPermissions",
        ]);
        command.current_dir(cwd).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());

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

        let mut child = command.spawn().context("failed to spawn `claude`")?;
        let stdin = child.stdin.take().context("missing child stdin")?;
        let stdout = child.stdout.take().context("missing child stdout")?;

        let events_tx_for_thread = events_tx.clone();
        let current_session_id_for_thread = Arc::clone(&current_session_id);

        let reader_thread = thread::spawn(move || {
            let mut reader = std::io::BufReader::new(stdout);
            let mut line = String::new();
            let mut state = ClaudeTranslationState::default();

            loop {
                line.clear();
                let n = match std::io::BufRead::read_line(&mut reader, &mut line) {
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

                let translated = match translate_claude_line(&mut state, trimmed) {
                    Ok(output) => output,
                    Err(_) => continue,
                };

                if let Some(session_id) = state
                    .session_id
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    if let Ok(mut current) = current_session_id_for_thread.lock() {
                        if current.as_deref() != Some(session_id) {
                            *current = Some(session_id.to_string());
                        }
                    }
                }

                for synthetic in translated.lines {
                    let _ = events_tx_for_thread.send(synthetic);
                }
            }
        });

        Ok(ClaudeProcess {
            child,
            stdin,
            reader_thread: Some(reader_thread),
        })
    }

    pub(crate) fn synthetic_start_response(
        &self,
        thread_id: &str,
        history_thread: Option<&Value>,
    ) -> String {
        let thread = history_thread
            .cloned()
            .unwrap_or_else(|| json!({ "id": thread_id }));
        json!({
            "jsonrpc": "2.0",
            "result": {
                "thread": thread
            },
        })
        .to_string()
    }

    fn send_stream_user_message(&self, text: &str) -> Result<String> {
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

        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Claude process lock poisoned"))?;
        process.stdin.write_all(line.as_bytes())?;
        process.stdin.write_all(b"\n")?;
        process.stdin.flush()?;

        Ok(json!({
            "jsonrpc": "2.0",
            "result": {}
        })
        .to_string())
    }

    fn recovery_launch_mode(&self) -> Result<ClaudeLaunchMode> {
        let current_session_id = self
            .current_session_id
            .lock()
            .map_err(|_| anyhow!("Claude session id lock poisoned"))?
            .clone();
        Ok(claude_recovery_launch_mode(
            &self.launch_mode,
            current_session_id.as_deref(),
        ))
    }

    fn respawn_process_locked(&self, process: &mut ClaudeProcess) -> Result<()> {
        let launch_mode = self.recovery_launch_mode()?;
        let runtime_settings = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude runtime settings lock poisoned"))?
            .clone();
        process.stop();
        *process = Self::spawn_process(
            &self.cwd,
            &launch_mode,
            &runtime_settings,
            self.events_tx.clone(),
            Arc::clone(&self.current_session_id),
        )?;
        Ok(())
    }

    fn ensure_runtime_settings(&self, model: Option<&str>, effort: Option<&str>) -> Result<()> {
        let desired = ClaudeRuntimeSettings {
            model: normalize_runtime_arg(model),
            effort: normalize_runtime_arg(effort),
        };
        let mut current = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude runtime settings lock poisoned"))?;
        if *current == desired {
            return Ok(());
        }
        *current = desired;
        drop(current);

        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Claude process lock poisoned"))?;
        self.respawn_process_locked(&mut process)
            .context("failed to apply Claude runtime settings")?;
        Ok(())
    }

    fn interrupt_process(&self) -> Result<()> {
        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Claude process lock poisoned"))?;
        let pid = process.child.id();
        let status = Command::new("kill")
            .arg("-INT")
            .arg(pid.to_string())
            .status()
            .context("failed to send SIGINT to Claude")?;
        if !status.success() {
            bail!("failed to interrupt Claude process");
        }

        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match process.child.try_wait() {
                Ok(Some(_)) => {
                    self.respawn_process_locked(&mut process)
                        .context("failed to restart Claude after interrupt")?;
                    break;
                }
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
                Ok(None) => break,
                Err(err) => {
                    return Err(err).context("failed to poll Claude process after SIGINT");
                }
            }
        }
        Ok(())
    }
}

impl ClaudeProcess {
    fn stop(&mut self) {
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
    }
}

// --- BackendClient trait impl ---

impl BackendClient for ClaudeClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        match method {
            "turn/start" | "turn/steer" => {
                self.ensure_runtime_settings(
                    params.get("model").and_then(Value::as_str),
                    params.get("effort").and_then(Value::as_str),
                )?;
                let text = params
                    .get("input")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("text"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .context("missing turn input text")?;
                self.send_stream_user_message(text)
            }
            "turn/interrupt" => {
                self.interrupt_process()?;
                let _ = self.events_tx.send(
                    json!({
                        "method": "turn/completed",
                        "params": {
                            "turn": {
                                "id": "claude-turn-interrupted",
                                "status": "interrupted"
                            }
                        }
                    })
                    .to_string(),
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "result": {}
                })
                .to_string())
            }
            other => bail!("unsupported Claude backend method: {other}"),
        }
    }

    fn respond(&self, request_id: &Value, result: Value) -> Result<()> {
        if let Some(follow_up) = claude_approval_follow_up_text(request_id, &result)? {
            self.send_stream_user_message(follow_up)?;
            return Ok(());
        }

        bail!("unsupported Claude approval request: {request_id}")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("Claude backend approvals are not implemented")
    }

    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.events_rx
            .take()
            .ok_or_else(|| anyhow!("Claude events receiver already taken"))
    }

    fn stop(&mut self) {
        if let Ok(mut process) = self.process.lock() {
            process.stop();
        }
    }
}

impl Drop for ClaudeClient {
    fn drop(&mut self) {
        BackendClient::stop(self);
    }
}
