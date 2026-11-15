//! Read exact child-turn identities with the existing native tokens and original deadlines.

use std::time::{Duration, Instant};

use carlos_proto::ids::TaskReadId;
use serde_json::{json, Value};

use super::panel_state::Request;
use crate::session::state::SessionState;
use crate::session::task_digest::hash_value;
use crate::session::task_windows::WindowContext;

/// Keep a small captured metadata proof and the native turn without retaining turn items.
#[derive(Debug)]
pub(in crate::session) struct RunRead {
    pub process: Option<super::panel_process_runs::ProcessRead>,
    pub window: Option<WindowContext>,
    pub id: TaskReadId,
    pub request: Option<Request>,
    pub started: Instant,
    root: String,
    epoch: u64,
    generation: u64,
    fingerprint: blake3::Hash,
    pub turn: Option<NativeTurn>,
    pub error: Option<String>,
}

/// Preserve the backend's exact turn id and status; display labels never become stop targets.
#[derive(Debug)]
pub(in crate::session) struct NativeTurn {
    pub id: String,
    pub status: String,
}

/// Start one addressed read, preserving pending readers and evicting only settled cache entries.
pub(in crate::session) fn begin_in(
    session: &mut SessionState,
    thread: &str,
    refresh: bool,
    window: Option<WindowContext>,
) -> Result<(), String> {
    let agent = agent_in(session, thread, window.as_ref())
        .ok_or("Task is no longer in the verified native window")?;
    let fingerprint = fingerprint(agent);
    if let Some(read) = session.codex_panel.runs.get(thread) {
        if read.matches_scope(session)
            && same_window(read.window.as_ref(), window.as_ref())
            && ((!refresh) || read.request.is_some() && read.matches(session, agent))
        {
            return Ok(());
        }
    }
    if !refresh && native_turn(agent).is_some() {
        return Ok(());
    }
    if session.codex_serial() >= u64::MAX - 1 {
        return Err("Native request tokens are exhausted; no task run read was sent".into());
    }
    reserve(session, thread)?;
    install(session, thread, thread, fingerprint, window, None);
    Ok(())
}

pub(super) fn reserve(session: &mut SessionState, key: &str) -> Result<(), String> {
    if session.codex_serial() >= u64::MAX - 1 {
        return Err("Native request tokens exhausted".into());
    }
    session.codex_panel.runs.remove(key);
    if session.codex_panel.runs.len() >= 8 {
        let oldest = session
            .codex_panel
            .runs
            .iter()
            .filter(|(_, read)| read.request.is_none())
            .min_by_key(|(_, read)| read.started)
            .map(|(id, _)| id.clone())
            .ok_or("At most eight native task run reads can be pending")?;
        session.codex_panel.runs.remove(&oldest);
    }
    Ok(())
}

pub(super) fn install(
    session: &mut SessionState,
    key: &str,
    thread: &str,
    fingerprint: blake3::Hash,
    window: Option<WindowContext>,
    process: Option<super::panel_process_runs::ProcessRead>,
) {
    let (method, params) = process
        .as_ref()
        .map(super::panel_process_runs::request)
        .unwrap_or_else(|| {
            (
                "thread/turns/list",
                json!({"threadId":thread,"sortDirection":"desc","itemsView":"notLoaded","limit":1}),
            )
        });
    let request = super::panel::request(session, method, params);
    let mut digest = session.task_scope();
    digest.update(b"runRead");
    digest.update(&request.token.to_le_bytes());
    let id = digest
        .finalize()
        .to_hex()
        .as_str()
        .parse()
        .expect("task read digest");
    session.codex_panel.runs.insert(
        key.into(),
        RunRead {
            process,
            window,
            id,
            request: Some(request),
            started: Instant::now(),
            root: session.thread_id().clone(),
            epoch: session.codex_scope().epoch(),
            generation: session.codex_connection().generation(),
            fingerprint,
            turn: None,
            error: None,
        },
    );
}

/// Accept a result only while its original task metadata, native root and connection still match.
pub(in crate::session) fn reply(
    session: &mut SessionState,
    thread: &str,
    result: &Value,
) -> Result<(), String> {
    let read = session
        .codex_panel
        .runs
        .get(thread)
        .ok_or("No addressed native task run read")?;
    let current = agent_in(session, thread, read.window.as_ref())
        .ok_or("Task left the native window while reading its run")?;
    if !read.matches(session, current) {
        return Err(
            "Task changed while reading its run; previous intent was not retargeted".into(),
        );
    }
    if read.started.elapsed() > Duration::from_secs(20) {
        return Err("Task run read exceeded 20 seconds".into());
    }
    let data = result["data"]
        .as_array()
        .ok_or("Malformed native task run page")?;
    if data.len() > 1 {
        return Err("Native task run read returned more than its one-turn limit".into());
    }
    let turn = data
        .first()
        .map(|turn| {
            let id = super::panel_pages::valid_id(&turn["id"])?;
            let status = turn["status"]
                .as_str()
                .filter(|status| !status.is_empty() && status.len() <= 128)
                .ok_or("Missing or oversized native task run status")?;
            Ok::<_, String>(NativeTurn {
                id: id.into(),
                status: status.into(),
            })
        })
        .transpose()?;
    let read = session
        .codex_panel
        .runs
        .get_mut(thread)
        .expect("addressed run read");
    read.request = None;
    read.turn = turn;
    read.error = None;
    Ok(())
}

/// Retain a bounded read error without changing another task or any native work.
pub(in crate::session) fn failed(session: &mut SessionState, thread: &str, message: &str) {
    if let Some(read) = session.codex_panel.runs.get_mut(thread) {
        read.request = None;
        read.turn = None;
        read.error = Some(crate::session::text::safe_text(
            &message.chars().take(256).collect::<String>(),
        ));
    }
}

/// Borrow one accepted run read while its original native metadata remains unchanged.
pub(in crate::session) fn ready<'a>(
    session: &'a SessionState,
    thread: &str,
) -> Option<&'a RunRead> {
    ready_in(session, thread, None)
}

pub(in crate::session) fn ready_in<'a>(
    session: &'a SessionState,
    thread: &str,
    window: Option<&WindowContext>,
) -> Option<&'a RunRead> {
    let read = session.codex_panel.runs.get(thread)?;
    (read.request.is_none()
        && read.error.is_none()
        && same_window(read.window.as_ref(), window)
        && read.matches(session, agent_in(session, thread, window)?))
    .then_some(read)
}

/// Read an already observed native turn with the same identity/status checks as a fresh page.
pub(in crate::session) fn native_turn(agent: &Value) -> Option<(&str, &str)> {
    let turn = &agent["lastObservedTurn"];
    let id = super::panel_pages::valid_id(&turn["id"]).ok()?;
    let status = turn["status"]
        .as_str()
        .filter(|status| !status.is_empty() && status.len() <= 128)?;
    Some((id, status))
}

fn agent<'a>(session: &'a SessionState, thread: &str) -> Option<&'a Value> {
    session
        .codex_panel
        .ownership
        .contains(session.thread_id(), session.codex_scope().epoch(), thread)
        .then(|| {
            session
                .codex_panel
                .agents
                .iter()
                .find(|agent| agent["id"] == thread)
        })
        .flatten()
}

fn agent_in<'a>(
    session: &'a SessionState,
    thread: &str,
    window: Option<&WindowContext>,
) -> Option<&'a Value> {
    if let Some(context) = window {
        let data = session.task_window_capture(context).ok()?;
        data.owners
            .contains(session.thread_id(), session.codex_scope().epoch(), thread)
            .then(|| data.agents.iter().find(|agent| agent["id"] == thread))
            .flatten()
    } else {
        agent(session, thread)
    }
}

pub(in crate::session) fn same_window(
    left: Option<&WindowContext>,
    right: Option<&WindowContext>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.window == right.window && std::sync::Arc::ptr_eq(&left.reader, &right.reader)
        }
        _ => false,
    }
}

fn fingerprint(agent: &Value) -> blake3::Hash {
    let mut digest = blake3::Hasher::new();
    digest.update(b"carlos.task-run-read.v1");
    hash_value(&mut digest, agent);
    digest.finalize()
}

impl RunRead {
    /// Fence cached results and read ids after a native root or connection changes.
    pub(in crate::session) fn matches_scope(&self, session: &SessionState) -> bool {
        self.root == *session.thread_id()
            && self.epoch == session.codex_scope().epoch()
            && self.generation == session.codex_connection().generation()
            && self
                .window
                .as_ref()
                .is_none_or(|context| session.task_window_capture(context).is_ok())
    }

    /// Preserve the original read's failure instead of silently following changed metadata.
    pub(in crate::session) fn read_error(
        &self,
        session: &SessionState,
        thread: &str,
    ) -> Option<String> {
        self.error.clone().or_else(|| {
            (!agent_in(session, thread, self.window.as_ref())
                .is_some_and(|agent| self.matches(session, agent)))
            .then(|| {
                "Task changed while reading its run; previous intent was not retargeted".into()
            })
        })
    }

    fn matches(&self, session: &SessionState, agent: &Value) -> bool {
        self.matches_scope(session) && self.fingerprint == fingerprint(agent)
    }
}
