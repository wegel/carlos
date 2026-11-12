//! Claude Agent SDK client backed by a compiled local controller and the native CLI.

mod client_controls;
mod client_prompt;
mod conversation;
#[cfg(test)]
mod conversation_tests;
#[cfg(test)]
mod event_audit_tests;
mod event_batch;
mod event_emitter;
#[cfg(all(test, unix))]
mod event_eof_support;
#[cfg(all(test, unix))]
mod event_eof_tests;
mod event_facts;
#[cfg(test)]
mod event_failure_tests;
#[cfg(test)]
mod event_fixture;
#[cfg(test)]
mod event_forwarder_support;
#[cfg(test)]
mod event_forwarder_tests;
mod event_handler;
mod event_history;
#[cfg(test)]
mod event_history_tests;
#[cfg(test)]
mod event_interrupt_pipe_tests;
mod event_lifecycle;
#[cfg(test)]
mod event_lifecycle_tests;
mod event_live;
mod event_normalizer;
#[cfg(test)]
mod event_normalizer_tests;
#[cfg(test)]
mod event_overflow_pipe_tests;
mod event_queue;
#[cfg(test)]
mod event_race_support;
#[cfg(test)]
mod event_race_tests;
#[cfg(test)]
mod event_source_tests;
mod event_sources;
#[cfg(test)]
mod event_test_support;
mod fact_transfer;
mod helper;
#[cfg(test)]
mod lifecycle_differential_tests;
#[cfg(test)]
mod lifecycle_reference;
#[cfg(test)]
mod lifecycle_reference_dispatch;
#[cfg(test)]
mod lifecycle_schedule_support;
mod message_visibility;
#[cfg(all(test, unix))]
mod pipe_test_support;
mod sessions;
mod snapshot;
#[cfg(test)]
mod test_support;
mod thinking;
mod tool_results;
mod translate;
mod transport;
#[cfg(all(test, target_os = "linux"))]
mod transport_drop_support;
#[cfg(all(test, target_os = "linux"))]
mod transport_drop_tests;
mod transport_reader;
mod transport_state;
#[cfg(test)]
mod transport_test_hooks;
#[cfg(all(test, unix))]
mod transport_tests;
mod transport_workers;
mod transport_writer;
mod types;
mod user_record;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendClient, BackendKind, EventReceiver, PromptOrigin, PromptPriority};

pub(crate) use self::fact_transfer::FactTransfer;
#[cfg(test)]
pub(crate) use self::message_visibility::is_child_report_notice;
pub(crate) use self::sessions::list_claude_sessions;
#[cfg(test)]
pub(crate) use self::test_support::{
    collect_live_forwarded_lines_for_test, session_misroute_event, FixtureNormalizer,
};
#[cfg(test)]
pub(crate) use self::translate::translate_claude_line;
pub(crate) use self::types::ClaudeLaunchMode;
#[cfg(test)]
pub(crate) use self::types::ClaudeTranslationState;

use self::event_queue::EventSender;
use self::helper::start_params;
use self::transport::ControllerTransport;
use self::types::{normalize_runtime_arg, ClaudeRuntimeSettings};

/// Owns the helper transport; startup callbacks reach the UI before startup completes.
pub(crate) struct ClaudeClient {
    handler: RefCell<self::event_handler::EventHandler>,
    cwd: PathBuf,
    transport: Arc<Mutex<Arc<ControllerTransport>>>,
    session_id: String,
    runtime_settings: Mutex<ClaudeRuntimeSettings>,
    events_tx: EventSender,
    events_rx: Option<EventReceiver>,
}

impl ClaudeClient {
    /// Start the helper and let the UI answer questions during SDK startup.
    pub(crate) fn start(cwd: &Path, launch: ClaudeLaunchMode) -> Result<Self> {
        let (events_tx, events_rx) = EventSender::channel(1024);
        let settings = ClaudeRuntimeSettings::default();
        let session_id = match &launch {
            ClaudeLaunchMode::Resume(id) => id.clone(),
            _ => sessions::fresh_session_id(),
        };
        let transport = Arc::new(ControllerTransport::start(events_tx.clone())?);
        sessions::begin_session(&transport, cwd, &launch, &settings, &session_id)?;
        Ok(Self {
            handler: RefCell::default(),
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

    fn helper_call(&self, method: &str, params: Value, timeout: Duration) -> Result<Value> {
        self.transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?
            .call(method, params, timeout)
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
            "turn/interrupt" => self.interrupt(timeout),
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
        if method == "history.continue" {
            return self.continue_history(&params);
        }
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

    fn handle_event(&self, line: &str) -> Option<Vec<String>> {
        Some(self.handler.borrow_mut().handle(line))
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

    fn take_events_rx(&mut self) -> Result<EventReceiver> {
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
        if let Ok(transport) = self.transport.lock() {
            transport.close();
        }
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

#[cfg(test)]
pub(crate) use event_test_support::client as receiver_client;
#[cfg(test)]
mod event_shutdown_tests;
#[cfg(test)]
mod event_tolerance_tests;
