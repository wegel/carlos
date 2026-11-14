//! Read bounded descendant windows, verify their ancestry, then enumerate loaded native terminals.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::panel_lineage::belongs;
pub use super::panel_lineage::parent;
use super::panel_scope::{PanelScope, PanelUpdate};
use super::panel_state::{Process, Snapshot, MAX_AGENTS, MAX_BYTES, MAX_PROCESSES};
use crate::session::state::SessionState;

/// Apply one complete native page without installing a partial metadata snapshot.
pub fn page(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    result: &Value,
) -> Result<(), String> {
    let read = session
        .codex_panel
        .snapshot
        .as_mut()
        .ok_or("No panel read")?;
    read.bytes = read.bytes.saturating_add(result.to_string().len());
    if read.bytes > MAX_BYTES || read.started.elapsed() > Duration::from_secs(20) {
        return Err("Panel read exceeds 8 MiB or 20 seconds; previous rows retained".into());
    }
    let method = read.request.method.clone();
    match method.as_str() {
        "thread/list" => agents(session, scope, update, result)?,
        "thread/read" => ancestor(session, scope, update, result)?,
        "thread/backgroundTerminals/list" => processes(session, scope, update, result)?,
        _ => return Err("Unexpected panel page method".into()),
    }
    Ok(())
}

fn agents(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    result: &Value,
) -> Result<(), String> {
    let data = result["data"]
        .as_array()
        .ok_or("Malformed descendant page")?;
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    let root = read.root.clone();
    for agent in data {
        let id = valid_id(&agent["id"])?;
        if read.agents.len() >= MAX_AGENTS {
            return Err("Too many agents in one native page".into());
        }
        if id == root {
            continue;
        }
        if read.lineage.contains_key(id) {
            return Err("Duplicate/root thread in descendant page; refresh required".into());
        }
        read.lineage.insert(id.into(), parent(agent));
        read.agents.push(agent.clone());
    }
    let cursor = cursor(result)?;
    if !cursor.is_null() && read.agents.len() < MAX_AGENTS {
        continue_page(session, cursor)?;
    } else {
        read.older = cursor;
        verify(session, scope, update)?;
    }
    Ok(())
}

fn ancestor(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    result: &Value,
) -> Result<(), String> {
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    let expected = valid_id(&read.request.params["threadId"])?;
    let thread = &result["thread"];
    if thread["id"] != expected {
        return Err("Ancestor read returned a different thread".into());
    }
    read.lineage.insert(expected.into(), parent(thread));
    verify(session, scope, update)
}

fn verify(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
) -> Result<(), String> {
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    let missing = read
        .lineage
        .values()
        .flatten()
        .find(|id| **id != read.root && !read.lineage.contains_key(*id))
        .cloned();
    if let Some(id) = missing {
        if read.checked.len() >= MAX_AGENTS {
            return Err("Parent chain exceeds panel bounds; relationship unavailable".into());
        }
        read.checked.insert(id.clone());
        let request = super::panel::request(
            session,
            "thread/read",
            json!({"threadId":id,"includeTurns":false}),
        );
        session
            .codex_panel
            .snapshot
            .as_mut()
            .expect("panel read")
            .request = request;
        return Ok(());
    }
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    let root = read.root.clone();
    read.agents
        .retain(|agent| belongs(agent["id"].as_str().unwrap_or(""), &root, &read.lineage));
    read.scopes.push_back(root);
    for agent in &read.agents {
        if matches!(
            agent["status"]["type"].as_str(),
            Some("active" | "idle" | "systemError")
        ) {
            read.scopes.push_back(valid_id(&agent["id"])?.into());
        }
    }
    next_scope(session, scope, update);
    Ok(())
}

fn processes(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    result: &Value,
) -> Result<(), String> {
    let data = result["data"]
        .as_array()
        .ok_or("Malformed background process page")?;
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    let thread = valid_id(&read.request.params["threadId"])?.to_string();
    for process in data {
        let id = valid_id(&process["processId"])?;
        if read.processes.len() >= MAX_PROCESSES {
            return Err("Background process list exceeds 512 records".into());
        }
        if read
            .processes
            .iter()
            .any(|row| row.thread == thread && row.data["processId"] == id)
        {
            return Err("Duplicate background process across pages; refresh required".into());
        }
        read.processes.push(Process {
            thread: thread.clone(),
            data: process.clone(),
        });
    }
    let cursor = cursor(result)?;
    if !cursor.is_null() {
        continue_page(session, cursor)?;
    } else {
        read.process_scopes.insert(thread, None);
        next_scope(session, scope, update);
    }
    Ok(())
}

fn continue_page(session: &mut SessionState, cursor: Value) -> Result<(), String> {
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    let key = cursor.as_str().ok_or("Invalid panel cursor")?;
    if !read.cursors.insert(key.into()) {
        return Err("Repeated panel cursor".into());
    }
    let request = read.request.clone();
    let request = super::panel::continue_read(session, request, cursor);
    session
        .codex_panel
        .snapshot
        .as_mut()
        .expect("panel read")
        .request = request;
    Ok(())
}

/// Continue loaded-thread process reads and publish only the completed window.
pub(crate) fn next_scope(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
) {
    let read = session.codex_panel.snapshot.as_mut().expect("panel read");
    if let Some(thread) = read.scopes.pop_front() {
        read.cursors.clear();
        let request = super::panel::request(
            session,
            "thread/backgroundTerminals/list",
            json!({"threadId":thread,"limit":1}),
        );
        session
            .codex_panel
            .snapshot
            .as_mut()
            .expect("panel read")
            .request = request;
        return;
    }
    let read = session
        .codex_panel
        .snapshot
        .take()
        .expect("complete panel read");
    session.codex_panel.refreshed = Some(Instant::now());
    if read.revision != session.codex_panel.revision {
        session.codex_panel.notice = "Agents changed while reading; refreshing…".into();
        session.codex_panel.refreshed = None;
        if scope.visible {
            super::panel::refresh(session, scope, false);
        }
        return;
    }
    if let Err(error) = publish(session, scope, update, read) {
        session.codex_panel.error = Some(error);
    }
}

fn publish(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    update: &mut PanelUpdate,
    read: Snapshot,
) -> Result<(), String> {
    if read.root != *session.thread_id() {
        return Err("Task read belongs to a retired root; previous rows retained".into());
    }
    let verified = read
        .lineage
        .keys()
        .filter(|id| belongs(id, &read.root, &read.lineage))
        .cloned();
    session.codex_panel.ownership =
        super::panel_state::VerifiedOwners::new(session.thread_id(), scope.native.epoch, verified)?;
    session.codex_panel.error = if read.errors.is_empty() {
        None
    } else {
        Some(read.errors.join("\n"))
    };
    session.codex_panel.agents = read.agents;
    session.codex_panel.processes = read.processes;
    session.codex_panel.process_scopes = read.process_scopes;
    session.codex_panel.older = read.older;
    session.codex_panel.older_window = read.older_window;
    session.codex_panel.notice = if session.codex_panel.older.is_null() && !read.older_window {
        "All verified descendants in this snapshot"
    } else {
        "Bounded agent window; l older, r latest"
    }
    .into();
    if scope.selected.is_none() {
        update.selected = session.codex_panel.ids().first().cloned();
    }
    super::task_status::panel_updated(session, scope);
    Ok(())
}

/// Accept bounded nonempty native IDs without deriving identity from labels.
pub(crate) fn valid_id(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 4096)
        .ok_or_else(|| "Missing or oversized native panel ID".into())
}

/// Accept only opaque string cursors or an explicit end of pages.
pub fn cursor(result: &Value) -> Result<Value, String> {
    match result.get("nextCursor") {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(value) => {
            valid_id(value)?;
            Ok(value.clone())
        }
    }
}
