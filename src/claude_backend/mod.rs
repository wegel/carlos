//! Claude CLI backend: process management, client lifecycle, and BackendClient trait impl.

mod exit_plan;
mod history;
mod process;
mod session_file;
mod session_fork;
mod snapshot;
mod translate;
mod types;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind};

pub(crate) use self::exit_plan::claude_approval_follow_up_text;
pub(crate) use self::history::{
    fork_claude_local_history, load_claude_local_history, ClaudeLocalHistory,
};
use self::process::{start_claude_process, ClaudeProcess};
#[cfg(test)]
pub(crate) use self::history::{
    fork_claude_local_history_from_projects_root, load_claude_local_history_from_projects_root,
};
#[cfg(test)]
pub(crate) use self::process::{
    build_claude_command_for_test, collect_live_forwarded_lines_for_test,
    probe_claude_startup_for_test,
};
#[cfg(test)]
pub(crate) use self::translate::translate_claude_line;
pub(crate) use self::types::{
    claude_model_catalog, claude_project_dir_name, claude_recovery_launch_mode,
    ClaudeLaunchMode, CLAUDE_PENDING_THREAD_ID,
};
#[cfg(test)]
pub(crate) use self::types::ClaudeTranslationState;

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

// --- Client construction and lifecycle ---

impl ClaudeClient {
    pub(crate) fn start(cwd: &Path, launch_mode: ClaudeLaunchMode) -> Result<Self> {
        let (events_tx, events_rx) = mpsc::channel::<String>();
        let current_session_id = Arc::new(Mutex::new(match &launch_mode {
            ClaudeLaunchMode::Resume(id) => Some(id.clone()),
            _ => None,
        }));
        let runtime_settings = ClaudeRuntimeSettings::default();
        let process = start_claude_process(
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
        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Claude process lock poisoned"))?;
        process.write_user_message(text)
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
        *process = start_claude_process(
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

    fn switch_to_session(&self, session_id: String) -> Result<()> {
        let previous_session_id = self
            .current_session_id
            .lock()
            .map_err(|_| anyhow!("Claude session id lock poisoned"))?
            .clone();
        {
            let mut current_session_id = self
                .current_session_id
                .lock()
                .map_err(|_| anyhow!("Claude session id lock poisoned"))?;
            *current_session_id = Some(session_id);
        }

        let mut process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Claude process lock poisoned"))?;
        if let Err(err) = self.respawn_process_locked(&mut process) {
            if let Ok(mut current_session_id) = self.current_session_id.lock() {
                *current_session_id = previous_session_id;
            }
            return Err(err).context("failed to switch Claude to forked session");
        }
        Ok(())
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

    fn fork_from_rewind(
        &self,
        thread_id: &str,
        request: crate::backend::RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        let history = fork_claude_local_history(&self.cwd, thread_id, request.keep_turns)?;
        self.switch_to_session(history.session_id.clone())?;
        Ok(self.synthetic_start_response(&history.session_id, Some(&history.thread)))
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
