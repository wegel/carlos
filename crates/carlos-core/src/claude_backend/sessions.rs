//! Reads SDK session summaries and starts replacement helpers without blocking native questions.

use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use uuid::Uuid;

use super::event_queue::EventSender;
use super::helper::start_params;
use super::transport::ControllerTransport;
use super::types::{ClaudeLaunchMode, ClaudeRuntimeSettings};

/// Ask the official SDK for every session page without launching a native query.
pub fn list_claude_sessions(cwd: &Path) -> Result<Vec<Value>> {
    let (events, _receiver) = EventSender::channel(1024);
    let helper = ControllerTransport::start(events)?;
    let mut sessions = Vec::new();
    let mut offset = 0;
    loop {
        let result = helper.call(
            "sessions.list",
            json!({"cwd": cwd, "offset": offset}),
            Duration::from_secs(20),
        )?;
        sessions.extend(
            result["sessions"]
                .as_array()
                .context("SDK session page has no sessions")?
                .iter()
                .cloned(),
        );
        let Some(next) = result["nextOffset"].as_u64() else {
            break;
        };
        offset = next;
    }
    Ok(sessions)
}

/// Start either a fresh query or paged history loading before its resume query.
pub(super) fn begin_session(
    transport: &ControllerTransport,
    cwd: &Path,
    launch: &ClaudeLaunchMode,
    settings: &ClaudeRuntimeSettings,
    fresh_id: &str,
) -> Result<()> {
    if let ClaudeLaunchMode::Resume(id) = launch {
        transport.request("sessions.info", json!({"cwd": cwd, "sessionId": id}))
    } else {
        let mut params = start_params(cwd, launch, settings)?;
        params["sessionId"] = json!(fresh_id);
        transport.request("start", params)
    }
}

/// Keep a fork worker tied to the helper and conversation captured before dispatch.
pub(super) struct ForkSource {
    pub(super) context: crate::backend_commands::ForkContext,
    pub(super) helper: Arc<ControllerTransport>,
    pub(super) epoch: u64,
    pub(super) permission: crate::dispatch_guard::DispatchGuard,
    pub(super) started: Option<mpsc::SyncSender<()>>,
}

impl ForkSource {
    fn check_current(&self, transport: &Arc<Mutex<Arc<ControllerTransport>>>) -> Result<()> {
        let current = transport
            .lock()
            .map_err(|_| anyhow!("Claude transport lock poisoned"))?;
        self.check(&current)
    }

    fn check(&self, current: &Arc<ControllerTransport>) -> Result<()> {
        self.context.liveness.check()?;
        anyhow::ensure!(
            Arc::ptr_eq(current, &self.helper),
            "Claude fork helper was replaced"
        );
        anyhow::ensure!(!self.helper.state.is_closed(), "Claude fork helper stopped");
        anyhow::ensure!(
            self.helper
                .conversation
                .lock()
                .map_err(|_| anyhow!("Claude conversation lock poisoned"))?
                .epoch
                == self.epoch,
            "Claude fork conversation has reset"
        );
        Ok(())
    }
}

/// Fork with the SDK, then replace helper identity before delivering any new frames.
pub(super) fn fork_session(
    transport: Arc<Mutex<Arc<ControllerTransport>>>,
    mut source: ForkSource,
    events: EventSender,
    cwd: std::path::PathBuf,
    settings: ClaudeRuntimeSettings,
    thread_id: String,
    before: String,
) -> Result<()> {
    let (started, receiver) = mpsc::sync_channel(1);
    source.started = Some(started);
    let operation = source.context.operation.clone();
    thread::spawn(move || {
        let result = replace_after_fork(
            &transport, source, &events, &cwd, &settings, &thread_id, &before,
        );
        if let Err(error) = result {
            let _ = events.send(
                json!({"method":"claude/forkFailed", "params":{"message":error.to_string(),"operation":operation}})
                    .to_string(),
            );
        }
    });
    receiver
        .recv_timeout(Duration::from_millis(500))
        .context("Claude fork did not start its guarded native write")
}

fn replace_after_fork(
    transport: &Arc<Mutex<Arc<ControllerTransport>>>,
    mut source: ForkSource,
    events: &EventSender,
    cwd: &Path,
    settings: &ClaudeRuntimeSettings,
    thread_id: &str,
    before: &str,
) -> Result<()> {
    source.check_current(transport)?;
    let original = source.helper.clone();
    let result = original.call_guarded_started(
        "sessions.forkBefore",
        json!({"cwd":cwd,"sessionId":thread_id,"beforeMessageId":before}),
        Duration::from_secs(20),
        source
            .permission
            .clone()
            .and(source.context.liveness.clone()),
        source.started.take(),
    )?;
    let id = result["sessionId"]
        .as_str()
        .context("SDK fork has no session ID")?;
    source.check_current(transport)?;
    let generation = events.generation + 1;
    let next_events = events.for_generation(generation);
    let replacement = Arc::new(original.replacement(next_events.clone())?);
    let launch = if result["fresh"] == true {
        ClaudeLaunchMode::New
    } else {
        ClaudeLaunchMode::Resume(id.to_string())
    };
    let mut slot = transport
        .lock()
        .map_err(|_| anyhow!("Claude transport lock poisoned"))?;
    source.check(&slot)?;
    next_events
        .send(json!({"method":"claude/sessionChanged","params":{"sessionId":id,"operation":source.context.operation}}).to_string())?;
    *slot = replacement.clone();
    drop(slot);
    if let Err(error) = begin_session(&replacement, cwd, &launch, settings, id) {
        let _ = next_events.send(
            json!({"method":"claude/sessionStartFailed", "params":{"message":error.to_string(),"operation":source.context.operation}})
                .to_string(),
        );
    }
    thread::spawn(move || original.stop());
    Ok(())
}

/// Choose an ID before native startup so fresh session history and UI identity agree.
pub(super) fn fresh_session_id() -> String {
    Uuid::new_v4().to_string()
}
