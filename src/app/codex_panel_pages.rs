//! Read bounded descendant windows, verify their ancestry, then enumerate loaded native terminals.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex_panel_state::{Process, Snapshot, MAX_AGENTS, MAX_BYTES, MAX_PROCESSES};
use super::state::AppState;

/// Apply one complete native page without installing a partial metadata snapshot.
pub(super) fn page(app: &mut AppState, result: &Value) -> Result<(), String> {
    let read = app.codex.panel.snapshot.as_mut().ok_or("No panel read")?;
    read.bytes = read.bytes.saturating_add(result.to_string().len());
    if read.bytes > MAX_BYTES || read.started.elapsed() > Duration::from_secs(20) {
        return Err("Panel read exceeds 8 MiB or 20 seconds; previous rows retained".into());
    }
    let method = read.request.method.clone();
    match method.as_str() {
        "thread/list" => agents(app, result)?,
        "thread/read" => ancestor(app, result)?,
        "thread/backgroundTerminals/list" => processes(app, result)?,
        _ => return Err("Unexpected panel page method".into()),
    }
    Ok(())
}

fn agents(app: &mut AppState, result: &Value) -> Result<(), String> {
    let data = result["data"]
        .as_array()
        .ok_or("Malformed descendant page")?;
    let root = app.thread_id.clone();
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
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
        continue_page(app, cursor)?;
    } else {
        read.older = cursor;
        verify(app)?;
    }
    Ok(())
}

fn ancestor(app: &mut AppState, result: &Value) -> Result<(), String> {
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
    let expected = valid_id(&read.request.params["threadId"])?;
    let thread = &result["thread"];
    if thread["id"] != expected {
        return Err("Ancestor read returned a different thread".into());
    }
    read.lineage.insert(expected.into(), parent(thread));
    verify(app)
}

fn verify(app: &mut AppState) -> Result<(), String> {
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
    let missing = read
        .lineage
        .values()
        .flatten()
        .find(|id| **id != app.thread_id && !read.lineage.contains_key(*id))
        .cloned();
    if let Some(id) = missing {
        if read.checked.len() >= MAX_AGENTS {
            return Err("Parent chain exceeds panel bounds; relationship unavailable".into());
        }
        read.checked.insert(id.clone());
        let request = super::codex_panel::request(
            app,
            "thread/read",
            json!({"threadId":id,"includeTurns":false}),
        );
        app.codex
            .panel
            .snapshot
            .as_mut()
            .expect("panel read")
            .request = request;
        return Ok(());
    }
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
    let root = app.thread_id.clone();
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
    next_scope(app);
    Ok(())
}

fn processes(app: &mut AppState, result: &Value) -> Result<(), String> {
    let data = result["data"]
        .as_array()
        .ok_or("Malformed background process page")?;
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
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
        continue_page(app, cursor)?;
    } else {
        read.process_scopes.insert(thread, None);
        next_scope(app);
    }
    Ok(())
}

fn continue_page(app: &mut AppState, cursor: Value) -> Result<(), String> {
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
    let key = cursor.as_str().ok_or("Invalid panel cursor")?;
    if !read.cursors.insert(key.into()) {
        return Err("Repeated panel cursor".into());
    }
    let request = read.request.clone();
    let request = super::codex_panel::continue_read(app, request, cursor);
    app.codex
        .panel
        .snapshot
        .as_mut()
        .expect("panel read")
        .request = request;
    Ok(())
}

/// Continue loaded-thread process reads and publish only the completed window.
pub(super) fn next_scope(app: &mut AppState) {
    let read = app.codex.panel.snapshot.as_mut().expect("panel read");
    if let Some(thread) = read.scopes.pop_front() {
        read.cursors.clear();
        let request = super::codex_panel::request(
            app,
            "thread/backgroundTerminals/list",
            json!({"threadId":thread,"limit":1}),
        );
        app.codex
            .panel
            .snapshot
            .as_mut()
            .expect("panel read")
            .request = request;
        return;
    }
    let read = app
        .codex
        .panel
        .snapshot
        .take()
        .expect("complete panel read");
    app.codex.panel.refreshed = Some(Instant::now());
    if read.revision != app.codex.panel.revision {
        app.codex.panel.notice = "Agents changed while reading; refreshing…".into();
        app.codex.panel.refreshed = None;
        if app.codex.panel.show {
            super::codex_panel::refresh(app, false);
        }
        return;
    }
    if let Err(error) = publish(app, read) {
        app.codex.panel.error = Some(error);
    }
}

fn publish(app: &mut AppState, read: Snapshot) -> Result<(), String> {
    let verified = read
        .lineage
        .keys()
        .filter(|id| belongs(id, &app.thread_id, &read.lineage))
        .cloned();
    app.codex.panel.ownership =
        super::codex_panel_state::VerifiedOwners::new(&app.thread_id, app.codex.epoch, verified)?;
    app.codex.panel.error = if read.errors.is_empty() {
        None
    } else {
        Some(read.errors.join("\n"))
    };
    app.codex.panel.agents = read.agents;
    app.codex.panel.processes = read.processes;
    app.codex.panel.process_scopes = read.process_scopes;
    app.codex.panel.older = read.older;
    app.codex.panel.older_window = read.older_window;
    app.codex.panel.notice = if app.codex.panel.older.is_null() && !read.older_window {
        "All verified descendants in this snapshot"
    } else {
        "Bounded agent window; l older, r latest"
    }
    .into();
    if app.codex.panel.selected.is_none() {
        app.codex.panel.selected = app.codex.panel.ids().first().cloned();
    }
    super::codex_task_status::panel_updated(app);
    Ok(())
}

/// Trust positive subagent source/parent metadata, including all ancestors to the selected root.
pub(super) fn parent(thread: &Value) -> Option<String> {
    thread["source"].get("subAgent")?;
    let source = thread["source"]["subAgent"]["thread_spawn"]["parent_thread_id"].as_str();
    let direct = thread["parentThreadId"].as_str();
    if source
        .zip(direct)
        .is_some_and(|(left, right)| left != right)
    {
        return None;
    }
    direct
        .or(source)
        .filter(|id| !id.is_empty() && id.len() <= 4096)
        .map(str::to_string)
}

fn belongs(
    id: &str,
    root: &str,
    parents: &std::collections::HashMap<String, Option<String>>,
) -> bool {
    let mut current = id;
    for _ in 0..MAX_AGENTS {
        let Some(Some(parent)) = parents.get(current) else {
            return false;
        };
        if parent == root {
            return true;
        }
        if parent == id {
            return false;
        }
        current = parent;
    }
    false
}

/// Accept bounded nonempty native IDs without deriving identity from labels.
pub(super) fn valid_id(value: &Value) -> Result<&str, String> {
    value
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 4096)
        .ok_or_else(|| "Missing or oversized native panel ID".into())
}

/// Accept only opaque string cursors or an explicit end of pages.
pub(super) fn cursor(result: &Value) -> Result<Value, String> {
    match result.get("nextCursor") {
        None | Some(Value::Null) => Ok(Value::Null),
        Some(value) => {
            valid_id(value)?;
            Ok(value.clone())
        }
    }
}
