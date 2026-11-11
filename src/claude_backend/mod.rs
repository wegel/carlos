//! Claude Agent SDK client backed by a compiled local controller and the native CLI.

mod conversation;
#[cfg(test)]
mod conversation_tests;
mod event_queue;
mod helper;
mod message_visibility;
mod sessions;
mod snapshot;
#[cfg(test)]
mod test_support;
mod thinking;
mod tool_results;
mod translate;
mod transport;
mod transport_reader;
#[cfg(all(test, unix))]
mod transport_tests;
mod transport_writer;
mod types;
mod user_record;

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::backend::{BackendClient, BackendKind, PromptOrigin, PromptPriority};

pub(crate) use self::conversation::process_message as is_process_message;
pub(crate) use self::message_visibility::{is_child_message, is_child_report_notice};
pub(crate) use self::sessions::list_claude_sessions;
#[cfg(test)]
pub(crate) use self::test_support::{
    collect_live_forwarded_lines_for_test, session_misroute_event,
};
pub(crate) use self::translate::translate_claude_line;
pub(crate) use self::types::ClaudeLaunchMode;
pub(crate) use self::types::ClaudeTranslationState;

use self::event_queue::EventSender;
use self::helper::start_params;
use self::transport::ControllerTransport;
use self::types::{normalize_runtime_arg, ClaudeRuntimeSettings};

/// Owns the helper transport; startup callbacks reach the UI before startup completes.
pub(crate) struct ClaudeClient {
    cwd: PathBuf,
    transport: Arc<Mutex<Arc<ControllerTransport>>>,
    session_id: String,
    runtime_settings: Mutex<ClaudeRuntimeSettings>,
    events_tx: EventSender,
    events_rx: Option<mpsc::Receiver<String>>,
}

impl ClaudeClient {
    /// Start the helper and let the UI answer questions during SDK startup.
    pub(crate) fn start(cwd: &Path, launch: ClaudeLaunchMode) -> Result<Self> {
        let (sender, events_rx) = mpsc::sync_channel(1024);
        let events_tx = EventSender::new(sender);
        let settings = ClaudeRuntimeSettings::default();
        let session_id = match &launch {
            ClaudeLaunchMode::Resume(id) => id.clone(),
            _ => sessions::fresh_session_id(),
        };
        let transport = Arc::new(ControllerTransport::start(events_tx.clone())?);
        sessions::begin_session(&transport, cwd, &launch, &settings, &session_id)?;
        Ok(Self {
            cwd: cwd.to_path_buf(),
            transport: Arc::new(Mutex::new(transport)),
            session_id,
            runtime_settings: Mutex::new(settings),
            events_tx,
            events_rx: Some(events_rx),
        })
    }

    /// Report the chosen session ID before native startup completes.
    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Wrap a session identity for the shared UI startup parser.
    pub(crate) fn synthetic_start_response(
        &self,
        thread_id: &str,
        history: Option<&Value>,
    ) -> String {
        json!({ "result": { "thread": history.cloned().unwrap_or_else(|| json!({ "id": thread_id })) } }).to_string()
    }

    fn current_events(&self) -> Result<EventSender> {
        let generation = self
            .transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .generation;
        Ok(self.events_tx.for_generation(generation))
    }

    fn helper_call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        self.transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .call(method, params, timeout)
    }

    fn submit(
        &self,
        params: &Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        if params.get("model").is_some() || params.get("effort").is_some() {
            self.apply_runtime_settings(params["model"].as_str(), params["effort"].as_str())?;
        }
        let text = params["input"][0]["text"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .context("missing prompt text")?;
        let uuid = Uuid::new_v4().to_string();
        let events = self.current_events()?;
        events.send(
            json!({ "method": "claude/promptSubmitted", "params": { "uuid": uuid,"text":text,"origin": match origin { PromptOrigin::Human => "human", PromptOrigin::Automatic => "automatic" },"priority":match priority { PromptPriority::Steer => "now", PromptPriority::Queue => "later" } } }).to_string(),
        )?;
        let result = self.helper_call("prompt", json!({ "uuid": uuid, "text": text,
            "origin": { "kind": match origin { PromptOrigin::Human => "human", PromptOrigin::Automatic => "auto-continuation" } },
            "priority": match priority { PromptPriority::Steer => "now", PromptPriority::Queue => "later" } }), timeout);
        if let Err(error) = result {
            let _ = events.send(
                json!({ "method": "claude/promptRejected", "params": { "uuid": uuid } })
                    .to_string(),
            );
            return Err(error);
        }
        Ok(json!({ "result": { "turn": { "id": uuid } } }).to_string())
    }
}

impl BackendClient for ClaudeClient {
    fn kind(&self) -> BackendKind {
        BackendKind::Claude
    }

    fn call(&self, method: &str, params: Value, timeout: Duration) -> Result<String> {
        match method {
            "turn/start" | "turn/steer" => {
                bail!("Claude prompts require explicit author and priority")
            }
            "thread/name/set" => {
                let result = self.helper_call(
                    "sessions.rename",
                    json!({"cwd":self.cwd,
                    "sessionId":params["threadId"],"title":params["name"]}),
                    timeout,
                )?;
                Ok(json!({"result":result}).to_string())
            }
            "turn/interrupt" => {
                let result = self.helper_call("interrupt", json!({}), timeout)?;
                Ok(json!({ "result": result }).to_string())
            }
            other => bail!("unsupported Claude method: {other}"),
        }
    }

    fn submit_prompt(
        &self,
        _method: &str,
        params: Value,
        origin: PromptOrigin,
        priority: PromptPriority,
        timeout: Duration,
    ) -> Result<String> {
        self.submit(&params, origin, priority, timeout)
    }

    fn apply_runtime_settings(&self, model: Option<&str>, effort: Option<&str>) -> Result<bool> {
        let desired = ClaudeRuntimeSettings {
            model: normalize_runtime_arg(model),
            effort: normalize_runtime_arg(effort).filter(|effort| effort != "auto"),
        };
        let mut current = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude settings lock poisoned"))?;
        if current.model != desired.model {
            self.helper_call(
                "model.set",
                json!({ "model": desired.model }),
                Duration::from_secs(10),
            )?;
            current.model = desired.model;
        }
        if current.effort != desired.effort {
            self.helper_call(
                "effort.set",
                json!({ "effort": desired.effort }),
                Duration::from_secs(10),
            )?;
            current.effort = desired.effort;
        }
        Ok(true)
    }

    fn request_native_control(&self, method: &str, mut params: Value) -> Result<()> {
        params["cwd"] = json!(self.cwd);
        if method == "start" {
            let settings = self
                .runtime_settings
                .lock()
                .map_err(|_| anyhow!("Claude settings lock poisoned"))?;
            let id = params["sessionId"]
                .as_str()
                .context("Resume has no session ID")?;
            params = start_params(
                &self.cwd,
                &ClaudeLaunchMode::Resume(id.to_string()),
                &settings,
            )?;
        }
        self.transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .request(method, params)
    }

    fn refresh_context(&self) -> Result<()> {
        self.transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .request("context", json!({}))
    }

    fn acknowledge_event(&self, length: usize) {
        self.events_tx.handled(length);
    }

    fn fork_from_rewind(
        &self,
        thread_id: &str,
        request: crate::backend::RewindForkRequest,
        _timeout: Duration,
    ) -> Result<String> {
        let before = request
            .before_turn_id
            .context("Select a prompt with a native UUID before editing")?;
        let settings = self
            .runtime_settings
            .lock()
            .map_err(|_| anyhow!("Claude settings lock poisoned"))?
            .clone();
        let generation = self
            .transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .generation;
        sessions::fork_session(
            self.transport.clone(),
            self.events_tx.for_generation(generation),
            self.cwd.clone(),
            settings,
            thread_id.to_string(),
            before,
        );
        Ok(json!({"result":{"claudeForkPending":true}}).to_string())
    }

    fn respond(&self, request_id: &Value, result: Value) -> Result<()> {
        let callback_id = request_id
            .as_str()
            .context("native callback ID must be a string")?;
        self.helper_call(
            "callback.respond",
            json!({ "callbackId": callback_id, "answer": result }),
            Duration::from_secs(10),
        )?;
        Ok(())
    }

    fn respond_error(&self, request_id: &Value, _code: i64, message: &str) -> Result<()> {
        bail!("Cannot answer native callback {request_id}: {message}")
    }

    fn take_events_rx(&mut self) -> Result<mpsc::Receiver<String>> {
        self.events_rx
            .take()
            .ok_or_else(|| anyhow!("Claude events already taken"))
    }

    fn stop(&mut self) {
        if let Ok(transport) = self.transport.lock() {
            transport.stop();
        }
    }
}

impl Drop for ClaudeClient {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
pub(crate) fn claude_interrupted_turn_notification(params: &Value) -> Result<String> {
    let thread_id = params["threadId"]
        .as_str()
        .context("missing interrupt threadId")?;
    let turn_id = params["turnId"]
        .as_str()
        .context("missing interrupt turnId")?;
    Ok(
        json!({ "method": "turn/completed", "params": { "threadId": thread_id,
        "turn": { "id": turn_id, "status": "interrupted" } } })
        .to_string(),
    )
}
