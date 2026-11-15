//! Grok ACP backend: process lifecycle and BackendClient implementation.

mod catalogue;
mod client;
mod commands;
mod history;
mod interrupt;
mod process;
#[cfg(test)]
pub(crate) mod receiver_test_support;
mod rewind;
mod startup;
mod translate;
mod types;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::protocol_params::ModelInfo;

use self::history::parse_grok_session_list;
use self::process::{GrokIo, GrokProcess};
use self::translate::mark_prompt_started;
use self::types::{
    normalize_runtime_arg, parse_grok_model_catalog, response_error_message, response_is_error,
    synthetic_start_response, token_usage_line,
};

pub(crate) use self::catalogue::visit_local_sessions;
pub use self::history::{load_grok_thread_summaries, GrokSessionSummary};
#[cfg(any(test, feature = "test-support"))]
pub use self::translate::translate_grok_notification_for_test;
pub use self::types::{
    grok_model_catalog, GrokLaunchMode, GrokRuntimeSettings, GrokTranslationState,
};

pub(crate) use self::types::GROK_PENDING_TURN_ID;

const SESSION_NEW_TIMEOUT: Duration = Duration::from_secs(60);
const SESSION_LOAD_TIMEOUT: Duration = Duration::from_secs(120);
const SESSION_LIST_TIMEOUT: Duration = Duration::from_secs(15);

// --- Client ---

/// Own a Grok process and expose its commands through the shared backend trait.
pub struct GrokClient {
    commands: Arc<GrokCommands>,
    process_owner: Mutex<GrokProcess>,
    events_rx: Option<mpsc::Receiver<String>>,
}

/// Share Grok session commands while the client owns the process and event consumer.
pub struct GrokCommands {
    revision: AtomicU64,
    stopped: AtomicBool,
    opening: AtomicBool,
    pub(super) cwd: PathBuf,
    session_id: Mutex<Option<String>>,
    runtime_settings: Mutex<GrokRuntimeSettings>,
    models: Mutex<Vec<ModelInfo>>,
    translation: Arc<Mutex<GrokTranslationState>>,
    process: Arc<GrokIo>,
    events_tx: crate::protocol_events::EventQueue,
}

impl std::ops::Deref for GrokClient {
    type Target = GrokCommands;
    fn deref(&self) -> &Self::Target {
        &self.commands
    }
}

impl GrokCommands {
    /// Ask Grok for its saved sessions in this working directory.
    pub fn list_sessions(&self) -> Result<Vec<GrokSessionSummary>> {
        let process = &self.process;
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

    /// Open the chosen new, saved or latest Grok session.
    pub fn open_session(&self, mode: &GrokLaunchMode) -> Result<String> {
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

    /// Copy the current Grok model choices, using the built-in list when needed.
    pub fn available_models(&self) -> Vec<ModelInfo> {
        self.models
            .lock()
            .map(|models| models.clone())
            .unwrap_or_else(|_| grok_model_catalog())
    }

    /// Wrap the Grok session id in the reply shape used by shared startup code.
    pub fn synthetic_start_response(&self, thread_id: &str) -> String {
        synthetic_start_response(thread_id)
    }

    /// Read the last replayed Grok token counts as a shared usage message.
    pub fn drain_replay_usage_line(&self) -> Option<String> {
        let state = self.translation.lock().ok()?;
        state.last_usage.map(token_usage_line)
    }

    /// Finish remaining replayed Grok items and return their shared backend messages.
    pub fn take_replay_tail(&self) -> Vec<String> {
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
        let process = &self.process;
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
            let _ = self.emit(
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
        self.revision.fetch_add(1, Ordering::SeqCst);
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

    fn apply_runtime_settings(
        &self,
        session_id: &str,
        model: Option<&str>,
        effort: Option<&str>,
        permission: crate::dispatch_guard::DispatchGuard,
    ) -> Result<()> {
        let desired = GrokRuntimeSettings {
            model: normalize_runtime_arg(model),
            effort: normalize_runtime_arg(effort),
        };
        let current = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Grok runtime settings lock poisoned"))?
            .clone();
        if current == desired {
            return Ok(());
        }
        let model_changed = current.model != desired.model;
        if model_changed {
            if let Some(model) = desired.model.as_deref() {
                let process = &self.process;
                let response = process.request_guarded(
                    "session/set_model",
                    json!({ "sessionId": session_id, "modelId": model }),
                    Duration::from_secs(10),
                    permission.clone(),
                )?;
                if let Some(message) = response_error_message(&response) {
                    bail!("Grok model change failed: {message}");
                }
            }
        }
        *self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Grok runtime settings lock poisoned"))? = desired;
        Ok(())
    }

    fn begin_prompt_translation(&self) -> Result<String> {
        let mut state = self
            .translation
            .lock()
            .map_err(|_| anyhow!("Grok translation lock poisoned"))?;
        mark_prompt_started(&mut state);
        // The reader uses this ID on its first fact, or when an empty result arrives.
        // The owner may process completion before the send worker reports its write.
        Ok(state
            .current_turn_id
            .clone()
            .unwrap_or_else(|| format!("grok-turn-{}", state.next_turn_seq.saturating_add(1))))
    }

    pub(super) fn emit(&self, line: String) -> Result<()> {
        self.events_tx.send(line).inspect_err(|_| {
            self.process.cancel();
        })
    }
}

#[cfg(test)]
#[path = "../tests/backend_owner_grok_tests.rs"]
mod backend_owner_grok_tests;

#[cfg(test)]
mod process_bounds_tests;

#[cfg(test)]
mod startup_tests;

#[cfg(test)]
mod prompt_completion_tests;
