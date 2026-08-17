//! Grok ACP backend: process lifecycle and BackendClient implementation.

mod history;
mod process;
mod rewind;
mod translate;
mod types;

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::ModelInfo;

use self::history::parse_grok_session_list;
use self::process::{start_grok_process, GrokProcess};
use self::translate::{interrupt_prompt_turn, mark_prompt_started};
use self::types::{
    normalize_runtime_arg, parse_grok_model_catalog, response_error_message, response_is_error,
    synthetic_start_response, token_usage_line, turn_input_text,
};

pub(crate) use self::history::{load_grok_thread_summaries, GrokSessionSummary};
pub(crate) use self::types::{
    grok_model_catalog, GrokLaunchMode, GrokRuntimeSettings, GrokTranslationState,
    GROK_PENDING_TURN_ID,
};

#[cfg(test)]
pub(crate) use self::history::{load_grok_thread_summaries_from, parse_iso_timestamp_secs};
#[cfg(test)]
pub(crate) use self::process::build_grok_command_for_test;
#[cfg(test)]
pub(crate) use self::rewind::{
    is_hidden_rewind_point, parse_rewind_points, resolve_rewind_target, GrokRewindPoint,
};
#[cfg(test)]
pub(crate) use self::translate::{
    finish_prompt_turn as finish_grok_prompt_turn_for_test, translate_grok_notification_for_test,
};

const SESSION_NEW_TIMEOUT: Duration = Duration::from_secs(60);
const SESSION_LOAD_TIMEOUT: Duration = Duration::from_secs(120);
const SESSION_LIST_TIMEOUT: Duration = Duration::from_secs(15);

// --- Client ---

pub(crate) struct GrokClient {
    pub(super) cwd: PathBuf,
    session_id: Mutex<Option<String>>,
    runtime_settings: Mutex<GrokRuntimeSettings>,
    models: Mutex<Vec<ModelInfo>>,
    translation: Arc<Mutex<GrokTranslationState>>,
    process: Mutex<GrokProcess>,
    events_tx: mpsc::Sender<String>,
    events_rx: Option<mpsc::Receiver<String>>,
}

impl GrokClient {
    pub(crate) fn start(cwd: &Path) -> Result<Self> {
        let (events_tx, events_rx) = mpsc::channel();
        let translation = Arc::new(Mutex::new(GrokTranslationState::default()));
        let runtime_settings = GrokRuntimeSettings::default();
        let process = start_grok_process(
            cwd,
            &runtime_settings,
            events_tx.clone(),
            Arc::clone(&translation),
        )?;
        Ok(Self {
            cwd: cwd.to_path_buf(),
            session_id: Mutex::new(None),
            runtime_settings: Mutex::new(runtime_settings),
            models: Mutex::new(grok_model_catalog()),
            translation,
            process: Mutex::new(process),
            events_tx,
            events_rx: Some(events_rx),
        })
    }

    pub(crate) fn list_sessions(&self) -> Result<Vec<GrokSessionSummary>> {
        let process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Grok process lock poisoned"))?;
        let response = process.request(
            "session/list",
            json!({ "cwd": self.cwd }),
            SESSION_LIST_TIMEOUT,
        )?;
        let parsed: Value = serde_json::from_str(&response).context("invalid session/list JSON")?;
        if let Some(error) = parsed.get("error") {
            bail!(
                "Grok session/list failed: {}",
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
            );
        }
        Ok(parse_grok_session_list(
            parsed.get("result").unwrap_or(&Value::Null),
            &self.cwd.to_string_lossy(),
        ))
    }

    pub(crate) fn open_session(&self, mode: &GrokLaunchMode) -> Result<String> {
        match mode {
            GrokLaunchMode::New => self.create_session(),
            GrokLaunchMode::Resume(session_id) => self.load_session(session_id),
            GrokLaunchMode::Continue => {
                let sessions = self.list_sessions()?;
                match sessions
                    .into_iter()
                    .max_by_key(|session| session.updated_at)
                {
                    Some(latest) => self.load_session(&latest.session_id),
                    None => self.create_session(),
                }
            }
        }
    }

    pub(crate) fn available_models(&self) -> Vec<ModelInfo> {
        self.models
            .lock()
            .map(|models| models.clone())
            .unwrap_or_else(|_| grok_model_catalog())
    }

    pub(crate) fn synthetic_start_response(&self, thread_id: &str) -> String {
        synthetic_start_response(thread_id)
    }

    pub(crate) fn drain_replay_usage_line(&self) -> Option<String> {
        let state = self.translation.lock().ok()?;
        state.last_usage.map(token_usage_line)
    }

    pub(crate) fn take_replay_tail(&self) -> Vec<String> {
        let Ok(mut state) = self.translation.lock() else {
            return Vec::new();
        };
        crate::grok_backend::translate::flush_replay_tail(&mut state).lines
    }

    fn create_session(&self) -> Result<String> {
        let response = self.session_request(
            "session/new",
            json!({
                "cwd": self.cwd,
                "mcpServers": [],
                "_meta": { "yoloMode": true },
            }),
            SESSION_NEW_TIMEOUT,
        )?;
        self.apply_session_result(&response)
    }

    fn load_session(&self, session_id: &str) -> Result<String> {
        {
            let mut state = self
                .translation
                .lock()
                .map_err(|_| anyhow!("Grok translation lock poisoned"))?;
            state.replaying = true;
        }
        let response = self.session_request(
            "session/load",
            json!({
                "sessionId": session_id,
                "cwd": self.cwd,
                "mcpServers": [],
                "_meta": { "yoloMode": true },
            }),
            SESSION_LOAD_TIMEOUT,
        );
        if let Ok(mut state) = self.translation.lock() {
            state.replaying = false;
        }
        let response = response?;
        if response_is_error(&response) {
            bail!(
                "failed to load Grok session {session_id}: {}",
                response_error_message(&response).unwrap_or_else(|| "unknown error".to_string())
            );
        }
        let _ = self.apply_session_result(&response);
        self.store_session_id(session_id.to_string());
        Ok(session_id.to_string())
    }

    pub(super) fn session_request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<String> {
        let process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Grok process lock poisoned"))?;
        process.request(method, params, timeout)
    }

    fn apply_session_result(&self, response: &str) -> Result<String> {
        let parsed: Value = serde_json::from_str(response).context("invalid Grok session JSON")?;
        if let Some(message) = response_error_message(response) {
            bail!("{message}");
        }
        let result = parsed.get("result").cloned().unwrap_or(Value::Null);
        if let Some(models) = result.get("models") {
            if let Ok(mut current) = self.models.lock() {
                *current = parse_grok_model_catalog(models);
            }
        }
        let session_id = result
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .context("missing Grok sessionId")?;
        self.store_session_id(session_id.clone());
        if let Some(model) = result
            .pointer("/models/currentModelId")
            .and_then(Value::as_str)
        {
            let _ = self.events_tx.send(
                json!({
                    "method": "thread/initialized",
                    "params": {
                        "thread": { "id": session_id },
                        "model": model,
                    }
                })
                .to_string(),
            );
        }
        Ok(session_id)
    }

    pub(super) fn store_session_id(&self, session_id: String) {
        if let Ok(mut current) = self.session_id.lock() {
            *current = Some(session_id.clone());
        }
        if let Ok(mut state) = self.translation.lock() {
            state.session_id = Some(session_id);
        }
    }

    pub(super) fn current_session_id(&self) -> Result<String> {
        self.session_id
            .lock()
            .map_err(|_| anyhow!("Grok session id lock poisoned"))?
            .clone()
            .context("Grok session is not open")
    }

    fn apply_runtime_settings(&self, model: Option<&str>, effort: Option<&str>) -> Result<()> {
        let desired = GrokRuntimeSettings {
            model: normalize_runtime_arg(model),
            effort: normalize_runtime_arg(effort),
        };
        let mut current = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Grok runtime settings lock poisoned"))?;
        if *current == desired {
            return Ok(());
        }
        let model_changed = current.model != desired.model;
        *current = desired.clone();
        drop(current);
        if model_changed {
            if let Some(model) = desired.model.as_deref() {
                let session_id = self.current_session_id()?;
                let process = self
                    .process
                    .lock()
                    .map_err(|_| anyhow!("Grok process lock poisoned"))?;
                let _ = process.request(
                    "session/set_model",
                    json!({ "sessionId": session_id, "modelId": model }),
                    Duration::from_secs(10),
                )?;
            }
        }
        Ok(())
    }

    fn begin_prompt_translation(&self) -> Result<()> {
        let mut state = self
            .translation
            .lock()
            .map_err(|_| anyhow!("Grok translation lock poisoned"))?;
        mark_prompt_started(&mut state);
        Ok(())
    }

    fn interrupt_session(&self) -> Result<String> {
        let lines = {
            let mut state = self
                .translation
                .lock()
                .map_err(|_| anyhow!("Grok translation lock poisoned"))?;
            interrupt_prompt_turn(&mut state).lines
        };
        for line in lines {
            let _ = self.events_tx.send(line);
        }
        let session_id = self.current_session_id()?;
        let process = self
            .process
            .lock()
            .map_err(|_| anyhow!("Grok process lock poisoned"))?;
        process.clear_in_flight_prompts();
        process.notify(
            "session/cancel",
            json!({ "sessionId": session_id, "cancelTrigger": "esc" }),
        )?;
        let _ = process.notify("x.ai/queue/clear", json!({ "sessionId": session_id }));
        Ok(json!({ "jsonrpc": "2.0", "result": {} }).to_string())
    }
}

// --- BackendClient ---

impl BackendClient for GrokClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Grok
    }

    fn call(&self, method: &str, params: Value, _timeout: Duration) -> Result<String> {
        match method {
            "turn/start" => {
                self.apply_runtime_settings(
                    params.get("model").and_then(Value::as_str),
                    params.get("effort").and_then(Value::as_str),
                )?;
                let text = turn_input_text(&params)?;
                let session_id = self.current_session_id()?;
                self.begin_prompt_translation()?;
                let process = self
                    .process
                    .lock()
                    .map_err(|_| anyhow!("Grok process lock poisoned"))?;
                process.start_prompt(json!({
                    "sessionId": session_id,
                    "prompt": [{ "type": "text", "text": text }],
                }))?;
                Ok(json!({ "jsonrpc": "2.0", "result": {} }).to_string())
            }
            "turn/steer" => bail!("Grok does not support mid-turn steer"),
            "turn/interrupt" => self.interrupt_session(),
            other => bail!("unsupported Grok backend method: {other}"),
        }
    }

    fn fork_from_rewind(
        &self,
        _thread_id: &str,
        request: crate::backend::RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        self.rewind_fork(request)
    }

    fn respond(&self, request_id: &Value, _result: Value) -> Result<()> {
        bail!("unsupported Grok approval request: {request_id}")
    }

    fn respond_error(&self, _request_id: &Value, _code: i64, _message: &str) -> Result<()> {
        bail!("Grok backend approvals are not implemented")
    }

    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.events_rx
            .take()
            .ok_or_else(|| anyhow!("Grok events receiver already taken"))
    }

    fn stop(&mut self) {
        if let Ok(mut process) = self.process.lock() {
            process.stop();
        }
    }
}

impl Drop for GrokClient {
    fn drop(&mut self) {
        BackendClient::stop(self);
    }
}
