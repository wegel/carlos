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
pub(crate) fn list_claude_sessions(cwd: &Path) -> Result<Vec<Value>> {
    let (sender, _receiver) = mpsc::sync_channel(1024);
    let helper = ControllerTransport::start(EventSender::new(sender))?;
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

/// Fork with the SDK, then replace helper identity before delivering any new frames.
pub(super) fn fork_session(
    transport: Arc<Mutex<Arc<ControllerTransport>>>,
    events: EventSender,
    cwd: std::path::PathBuf,
    settings: ClaudeRuntimeSettings,
    thread_id: String,
    before: String,
) {
    thread::spawn(move || {
        let result = replace_after_fork(&transport, &events, &cwd, &settings, &thread_id, &before);
        if let Err(error) = result {
            let generation = transport
                .lock()
                .map(|helper| helper.generation)
                .unwrap_or(events.generation);
            let _ = events.for_generation(generation).send(
                json!({"method":if generation == events.generation {"claude/forkFailed"} else {"claude/sessionStartFailed"}, "params":{"message":error.to_string()}})
                    .to_string(),
            );
        }
    });
}

fn replace_after_fork(
    transport: &Arc<Mutex<Arc<ControllerTransport>>>,
    events: &EventSender,
    cwd: &Path,
    settings: &ClaudeRuntimeSettings,
    thread_id: &str,
    before: &str,
) -> Result<()> {
    let original = transport
        .lock()
        .map_err(|_| anyhow!("Claude transport lock poisoned"))?
        .clone();
    let result = original.call(
        "sessions.forkBefore",
        json!({"cwd":cwd,"sessionId":thread_id,"beforeMessageId":before}),
        Duration::from_secs(20),
    )?;
    let id = result["sessionId"]
        .as_str()
        .context("SDK fork has no session ID")?;
    let generation = events.generation + 1;
    let next_events = events.for_generation(generation);
    let replacement = Arc::new(ControllerTransport::start(next_events.clone())?);
    let launch = if result["fresh"] == true {
        ClaudeLaunchMode::New
    } else {
        ClaudeLaunchMode::Resume(id.to_string())
    };
    let mut slot = transport
        .lock()
        .map_err(|_| anyhow!("Claude transport lock poisoned"))?;
    next_events
        .send(json!({"method":"claude/sessionChanged","params":{"sessionId":id}}).to_string())?;
    *slot = replacement.clone();
    drop(slot);
    let started = begin_session(&replacement, cwd, &launch, settings, id);
    thread::spawn(move || original.stop());
    started
}

/// Choose an ID before native startup so fresh session history and UI identity agree.
pub(super) fn fresh_session_id() -> String {
    Uuid::new_v4().to_string()
}
