//! Refresh only known terminal owners while background work can affect the compact task badge.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex_panel_state::{Process, Request};
use super::state::AppState;
use crate::backend::BackendClient;

#[path = "codex_task_pages.rs"]
mod pages;

#[derive(Default)]
/// Keep one targeted read separate from the user's F2 selection and details.
pub(super) struct TaskStatus {
    dirty: BTreeSet<String>,
    read: Option<Read>,
    next: Option<Instant>,
    pub(super) uncertain: HashSet<String>,
    pub(super) errors: HashMap<String, String>,
    revisions: HashMap<String, u64>,
    allowed: HashMap<String, Instant>,
}

struct Read {
    request: Request,
    started: Instant,
    bytes: usize,
    items: Vec<Process>,
    cursors: HashSet<String>,
    revision: u64,
    generation: u64,
    sent: bool,
}

/// Native command IDs justify a read; idle/completed observations invalidate older reads.
pub(super) fn observed(app: &mut AppState, method: &str, params: &Value) {
    let Some(owner) = params["threadId"]
        .as_str()
        .filter(|id| super::task_badge::owns(app, id))
    else {
        return;
    };
    let item = &params["item"];
    let command = matches!(method, "item/started" | "item/completed")
        && item["type"] == "commandExecution"
        && (item["processId"].as_str().is_some_and(|id| !id.is_empty())
            || app
                .codex
                .panel
                .processes
                .iter()
                .any(|row| row.thread == owner && row.data["itemId"] == item["id"]));
    let settled = method == "turn/completed"
        || method == "thread/closed"
        || method == "thread/status/changed" && params["status"]["type"] != "active";
    if command
        || settled
            && app
                .codex
                .panel
                .processes
                .iter()
                .any(|row| row.thread == owner)
    {
        invalidate(app, owner, true);
    }
}

/// A successful complete F2 snapshot supersedes older targeted reads without changing selection.
pub(super) fn panel_updated(app: &mut AppState) {
    let scopes = &app.codex.panel.process_scopes;
    let status = &mut app.codex.task_status;
    if status.read.as_ref().is_some_and(|read| {
        read.request.params["threadId"]
            .as_str()
            .is_some_and(|owner| scopes.contains_key(owner))
    }) {
        status.read = None;
    }
    status.dirty.retain(|owner| !scopes.contains_key(owner));
    for (owner, error) in &app.codex.panel.process_scopes {
        if let Some(error) = error {
            status.uncertain.insert(owner.clone());
            status.errors.insert(owner.clone(), error.clone());
        } else {
            status.uncertain.remove(owner);
            status.errors.remove(owner);
        }
    }
    let live = !owners(app).is_empty();
    app.codex.task_status.next = live.then(|| Instant::now() + Duration::from_secs(3));
}

/// A native terminated result retires only that exact panel process and fences old snapshots.
pub(super) fn terminated(app: &mut AppState, owner: &str, process: &str, item: Option<&str>) {
    app.codex.panel.processes.retain(|row| {
        !(row.thread == owner
            && row.data["processId"] == process
            && row.data["itemId"].as_str() == item)
    });
    invalidate(app, owner, true);
}

/// Dispatch at most one bounded read; a known empty owner stops polling immediately.
pub(super) fn tick(client: &dyn BackendClient, app: &mut AppState, now: Instant) -> bool {
    if !connected(app) {
        return false;
    }
    prune(app);
    let mut changed = false;
    if app
        .codex
        .task_status
        .read
        .as_ref()
        .is_some_and(|read| now.saturating_duration_since(read.started) >= Duration::from_secs(20))
    {
        failed(app, "Background process refresh timed out; F2 can retry");
        changed = true;
    }
    schedule(app, now);
    let Some(read) = app
        .codex
        .task_status
        .read
        .as_mut()
        .filter(|read| !read.sent)
    else {
        return changed;
    };
    read.sent = true;
    let context = json!({"operation":"taskStatus","root":app.thread_id,"epoch":app.codex.epoch,
        "generation":read.generation,"token":read.request.token,
        "timeoutMs":Duration::from_secs(20).saturating_sub(now.saturating_duration_since(read.started)).as_millis().clamp(1,5000)});
    if let Err(error) = client.request_control_with_context(
        &read.request.method,
        read.request.params.clone(),
        context,
    ) {
        failed(
            app,
            &format!("Background process refresh failed: {error}; F2 can retry"),
        );
    }
    true
}

/// Apply only a live owner's exact read, after all pages and the original budget validate.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    let context = &params["requestContext"];
    if context["operation"] != "taskStatus" {
        return false;
    }
    if !connected(app)
        || context["root"] != app.thread_id
        || context["epoch"] != app.codex.epoch
        || context["generation"] != app.codex.connection.generation
    {
        return true;
    }
    let Some(read) = app
        .codex
        .task_status
        .read
        .as_ref()
        .filter(|read| context["token"] == read.request.token)
    else {
        return true;
    };
    let owner = read.request.params["threadId"]
        .as_str()
        .expect("target owner")
        .to_string();
    if !super::task_badge::owns(app, &owner) {
        app.codex.task_status.read = None;
        return true;
    }
    if read.revision != *app.codex.task_status.revisions.get(&owner).unwrap_or(&0) {
        app.codex.task_status.read = None;
        app.codex.task_status.dirty.insert(owner);
        return true;
    }
    if let Err(error) = pages::accept_page(app, &params["response"]) {
        failed(app, &error);
    }
    true
}

/// Wake for real pending work only; no timer remains after empty or unavailable process facts.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    if !connected(app) {
        return None;
    }
    let status = &app.codex.task_status;
    if let Some(read) = &status.read {
        return Some(if read.sent {
            (read.started + Duration::from_secs(20)).saturating_duration_since(now)
        } else {
            Duration::ZERO
        });
    }
    if app.codex.panel.snapshot.is_some() {
        return None;
    }
    if !status.dirty.is_empty() {
        return status
            .dirty
            .iter()
            .map(|owner| {
                status
                    .allowed
                    .get(owner)
                    .map_or(Duration::ZERO, |at| at.saturating_duration_since(now))
            })
            .min();
    }
    status.next.map(|at| at.saturating_duration_since(now))
}

fn schedule(app: &mut AppState, now: Instant) {
    if app.codex.task_status.read.is_none() && app.codex.panel.snapshot.is_none() {
        if app.codex.task_status.next.is_some_and(|at| now >= at) {
            let owners = owners(app);
            app.codex.task_status.dirty.extend(owners);
            app.codex.task_status.next = None;
        }
        let next = app
            .codex
            .task_status
            .dirty
            .iter()
            .find(|owner| {
                super::task_badge::owns(app, owner)
                    && app
                        .codex
                        .task_status
                        .allowed
                        .get(*owner)
                        .is_none_or(|at| now >= *at)
            })
            .cloned();
        if let Some(owner) = next {
            begin(app, &owner, now);
        }
    }
}

fn begin(app: &mut AppState, owner: &str, now: Instant) {
    app.codex.task_status.dirty.remove(owner);
    app.codex
        .task_status
        .allowed
        .insert(owner.into(), now + Duration::from_secs(3));
    app.codex.serial = app.codex.serial.saturating_add(1);
    app.codex.task_status.read = Some(Read {
        request: Request {
            token: app.codex.serial,
            method: "thread/backgroundTerminals/list".into(),
            params: json!({"threadId":owner,"limit":1,"cursor":null}),
        },
        started: now,
        bytes: 0,
        items: Vec::new(),
        cursors: HashSet::new(),
        revision: *app.codex.task_status.revisions.get(owner).unwrap_or(&0),
        generation: app.codex.connection.generation,
        sent: false,
    });
}

fn invalidate(app: &mut AppState, owner: &str, dirty: bool) {
    *app.codex
        .task_status
        .revisions
        .entry(owner.into())
        .or_default() += 1;
    app.codex.panel.revision = app.codex.panel.revision.saturating_add(1);
    app.codex.task_status.uncertain.insert(owner.into());
    if dirty {
        app.codex.task_status.dirty.insert(owner.into());
    }
}

fn failed(app: &mut AppState, error: &str) {
    if let Some(read) = app.codex.task_status.read.take() {
        if let Some(owner) = read.request.params["threadId"].as_str() {
            app.codex.task_status.uncertain.insert(owner.into());
            app.codex
                .task_status
                .errors
                .insert(owner.into(), error.into());
        }
    }
    if owners(app).is_empty() {
        app.codex.task_status.next = None;
    }
}

fn owners(app: &AppState) -> BTreeSet<String> {
    app.codex
        .panel
        .processes
        .iter()
        .filter(|row| {
            super::task_badge::owns(app, &row.thread)
                && !app.codex.task_status.uncertain.contains(&row.thread)
        })
        .map(|row| row.thread.clone())
        .collect()
}

fn connected(app: &AppState) -> bool {
    app.codex.enabled && !app.codex.closed && !app.codex.connection.rebuilding
}

fn prune(app: &mut AppState) {
    let mut valid: HashSet<String> = app
        .codex
        .panel
        .ownership
        .ids()
        .into_iter()
        .filter(|owner| super::task_badge::owns(app, owner))
        .collect();
    valid.insert(app.thread_id.clone());
    let status = &mut app.codex.task_status;
    status.dirty.retain(|owner| valid.contains(owner));
    status.uncertain.retain(|owner| valid.contains(owner));
    status.errors.retain(|owner, _| valid.contains(owner));
    status.revisions.retain(|owner, _| valid.contains(owner));
    status.allowed.retain(|owner, _| valid.contains(owner));
    if owners(app).is_empty()
        && app.codex.task_status.read.is_none()
        && app.codex.task_status.dirty.is_empty()
    {
        app.codex.task_status.next = None;
    }
}
