//! Verify an older process's original item and PID through the existing addressed run-read slots.

use std::collections::HashSet;
use std::time::Duration;

use carlos_proto::ids::TaskId;
use serde_json::{json, Number, Value};

use super::panel_runs::{self, RunRead};
use super::panel_state::MAX_BYTES;
use crate::session::state::SessionState;
use crate::session::task_stop_target::process_identity;
use crate::session::task_windows::WindowContext;

#[derive(Debug)]
pub(in crate::session) struct ProcessRead {
    pub owner: String,
    process: String,
    item: Option<String>,
    pid: Option<Number>,
    revision: u64,
    cursors: HashSet<blake3::Hash>,
    bytes: usize,
    rows: usize,
    pub verified: bool,
}

pub(in crate::session) fn key(task: &TaskId) -> String {
    format!("process:{task}")
}

pub(in crate::session) fn begin(
    session: &mut SessionState,
    window: &WindowContext,
    task: &TaskId,
) -> Result<(), String> {
    let data = session
        .task_window_capture(window)
        .map_err(|error| error.message)?;
    let row = data
        .processes
        .iter()
        .find(|row| {
            crate::session::task_rows::TaskRef::WindowProcess { row, error: None }.id(session)
                == *task
        })
        .ok_or("Process left the captured native window")?;
    let (item, pid) =
        process_identity(&row.data).ok_or("Original process item/PID is unsupported")?;
    let process = ProcessRead {
        owner: row.thread.clone(),
        process: row.data["processId"]
            .as_str()
            .ok_or("Missing native process id")?
            .into(),
        item: item.map(str::to_string),
        pid: pid.cloned(),
        revision: session.codex_task_status.owner_revision(&row.thread),
        cursors: HashSet::new(),
        bytes: 0,
        rows: 0,
        verified: false,
    };
    let key = key(task);
    if session.codex_panel.runs.get(&key).is_some_and(|read| {
        read.request.is_some()
            && read.matches_scope(session)
            && panel_runs::same_window(read.window.as_ref(), Some(window))
    }) {
        return Ok(());
    }
    panel_runs::reserve(session, &key)?;
    let owner = process.owner.clone();
    panel_runs::install(
        session,
        &key,
        &owner,
        blake3::hash(b"process"),
        Some(window.clone()),
        Some(process),
    );
    Ok(())
}

pub(in crate::session) fn page(
    session: &mut SessionState,
    key: &str,
    result: &Value,
) -> Result<(), String> {
    let read = session
        .codex_panel
        .runs
        .get(key)
        .ok_or("No original process run read")?;
    if !read.matches_scope(session)
        || !current(read, session)
        || read.started.elapsed() > Duration::from_secs(20)
    {
        return Err("Original process proof retired while reading; no stop sent".into());
    }
    let data = result["data"]
        .as_array()
        .ok_or("Malformed native process page")?;
    let cursor = super::panel_reader::cursor(result)?;
    let read = session
        .codex_panel
        .runs
        .get_mut(key)
        .expect("original process read");
    let process = read.process.as_mut().expect("process run target");
    process.bytes = process
        .bytes
        .saturating_add(crate::session::task_window_bytes::encoded_bytes(result));
    process.rows = process.rows.saturating_add(data.len());
    if process.bytes > MAX_BYTES || process.rows > 512 {
        return Err("Process proof exceeds native read bounds".into());
    }
    for row in data {
        if row["processId"] != process.process {
            continue;
        }
        if process.verified {
            return Err("Native process page repeats the original target".into());
        }
        let (item, pid) =
            process_identity(row).ok_or("Native process item/PID became unsupported")?;
        if item != process.item.as_deref() || pid != process.pid.as_ref() {
            return Err("Original process item/PID changed; stop intent was not retargeted".into());
        }
        process.verified = true;
    }
    advance(session, key, cursor)
}

fn advance(session: &mut SessionState, key: &str, cursor: Value) -> Result<(), String> {
    let read = session
        .codex_panel
        .runs
        .get_mut(key)
        .expect("original process read");
    let process = read.process.as_mut().expect("process run target");
    if cursor.is_null() {
        read.request = None;
        if !process.verified {
            return Err("Original process is no longer present; no stop sent".into());
        }
        return Ok(());
    }
    let cursor = cursor.as_str().expect("validated cursor");
    if process.cursors.len() >= 512 {
        return Err("Native process proof exceeds its 512-page bound".into());
    }
    if !process.cursors.insert(blake3::hash(cursor.as_bytes())) {
        return Err("Repeated native process proof cursor".into());
    }
    let previous = read.request.clone().expect("pending process proof");
    let request = super::panel::continue_read(session, previous, Value::String(cursor.into()));
    session
        .codex_panel
        .runs
        .get_mut(key)
        .expect("original process read")
        .request = Some(request);
    Ok(())
}

pub(in crate::session) fn current(read: &RunRead, session: &SessionState) -> bool {
    read.process.as_ref().is_some_and(|process| {
        process.revision == session.codex_task_status.owner_revision(&process.owner)
    })
}

pub(in crate::session) fn verified(read: &RunRead, session: &SessionState) -> bool {
    read.matches_scope(session)
        && current(read, session)
        && read.request.is_none()
        && read.error.is_none()
        && read
            .process
            .as_ref()
            .is_some_and(|process| process.verified)
}

pub(in crate::session) fn reconcile(
    session: &mut SessionState,
    owner: &str,
    rows: &[super::panel_state::Process],
) {
    for read in session.codex_panel.runs.values_mut() {
        let Some(process) = read
            .process
            .as_ref()
            .filter(|process| process.owner == owner)
        else {
            continue;
        };
        let matches = rows.iter().any(|row| {
            row.thread == owner
                && row.data["processId"] == process.process
                && process_identity(&row.data).is_some_and(|(item, pid)| {
                    item == process.item.as_deref() && pid == process.pid.as_ref()
                })
        });
        if !matches {
            read.error =
                Some("New native process facts retired the original item/PID proof".into());
        }
    }
}

pub(super) fn request(process: &ProcessRead) -> (&'static str, Value) {
    (
        "thread/backgroundTerminals/list",
        json!({"threadId":process.owner,"limit":1}),
    )
}
