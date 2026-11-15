//! Refresh only known terminal owners while background work can affect the compact task badge.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::panel_scope::PanelScope;
use super::panel_state::{Process, Request};
use crate::backend::BackendClient;
use crate::session::state::SessionState;
use schedule::{connected, failed, invalidate, owners, prune, schedule};

#[path = "task_schedule.rs"]
mod schedule;

#[path = "task_pages.rs"]
mod pages;

#[derive(Debug, Default)]
/// Keep one targeted read separate from the user's F2 selection and details.
pub struct TaskStatus {
    dirty: BTreeSet<String>,
    read: Option<Read>,
    next: Option<Instant>,
    pub(in crate::session) uncertain: HashSet<String>,
    pub(in crate::session) errors: HashMap<String, String>,
    revisions: HashMap<String, u64>,
    allowed: HashMap<String, Instant>,
}

#[derive(Debug)]
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
pub fn observed(session: &mut SessionState, scope: PanelScope<'_>, method: &str, params: &Value) {
    if !matches!(
        method,
        "item/started"
            | "item/completed"
            | "turn/completed"
            | "thread/closed"
            | "thread/status/changed"
    ) {
        return;
    }
    let window_owners = session.window_process_owners();
    let Some(owner) = params["threadId"]
        .as_str()
        .filter(|id| super::task_badge::owns(session, scope, id) || window_owners.contains(*id))
    else {
        return;
    };
    let item = &params["item"];
    let command = matches!(method, "item/started" | "item/completed")
        && item["type"] == "commandExecution"
        && (item["processId"].as_str().is_some_and(|id| !id.is_empty())
            || window_owners.contains(owner)
            || session
                .codex_panel
                .processes
                .iter()
                .any(|row| row.thread == owner && row.data["itemId"] == item["id"]));
    let settled = method == "turn/completed"
        || method == "thread/closed"
        || method == "thread/status/changed" && params["status"]["type"] != "active";
    if command
        || settled
            && (window_owners.contains(owner)
                || session
                    .codex_panel
                    .processes
                    .iter()
                    .any(|row| row.thread == owner))
    {
        session.task_stops.retire_process_owner(owner);
        invalidate(session, owner, true);
    }
}

/// A successful complete F2 snapshot supersedes older targeted reads without changing selection.
pub fn panel_updated(session: &mut SessionState, scope: PanelScope<'_>) {
    let scopes = &session.codex_panel.process_scopes;
    let status = &mut session.codex_task_status;
    if status.read.as_ref().is_some_and(|read| {
        read.request.params["threadId"]
            .as_str()
            .is_some_and(|owner| scopes.contains_key(owner))
    }) {
        status.read = None;
    }
    status.dirty.retain(|owner| !scopes.contains_key(owner));
    for (owner, error) in &session.codex_panel.process_scopes {
        if let Some(error) = error {
            status.uncertain.insert(owner.clone());
            status.errors.insert(owner.clone(), error.clone());
        } else {
            status.uncertain.remove(owner);
            status.errors.remove(owner);
        }
    }
    let live = !owners(session, scope).is_empty();
    session.codex_task_status.next = live.then(|| Instant::now() + Duration::from_secs(3));
}

/// A native terminated result retires only that exact panel process and fences old snapshots.
pub fn terminated(session: &mut SessionState, owner: &str, process: &str, item: Option<&str>) {
    session.codex_panel.processes.retain(|row| {
        !(row.thread == owner
            && row.data["processId"] == process
            && row.data["itemId"].as_str() == item)
    });
    invalidate(session, owner, true);
}

/// Dispatch at most one bounded read; a known empty owner stops polling immediately.
pub fn tick(
    client: &dyn BackendClient,
    session: &mut SessionState,
    scope: PanelScope<'_>,
    now: Instant,
) -> bool {
    if !connected(scope) {
        return false;
    }
    prune(session, scope);
    let mut changed = false;
    if session
        .codex_task_status
        .read
        .as_ref()
        .is_some_and(|read| now.saturating_duration_since(read.started) >= Duration::from_secs(20))
    {
        failed(
            session,
            scope,
            "Background process refresh timed out; F2 can retry",
        );
        changed = true;
    }
    schedule(session, scope, now);
    let Some(read) = session
        .codex_task_status
        .read
        .as_mut()
        .filter(|read| !read.sent)
    else {
        return changed;
    };
    read.sent = true;
    let context = json!({"operation":"taskStatus","root":&session.thread_id,"epoch":scope.native.epoch,
        "generation":read.generation,"token":read.request.token,
        "timeoutMs":Duration::from_secs(20).saturating_sub(now.saturating_duration_since(read.started)).as_millis().clamp(1,5000)});
    if let Err(error) = client.request_control_with_context(
        &read.request.method,
        read.request.params.clone(),
        context,
    ) {
        failed(
            session,
            scope,
            &format!("Background process refresh failed: {error}; F2 can retry"),
        );
    }
    true
}

/// Apply only a live owner's exact read, after all pages and the original budget validate.
pub fn reply(session: &mut SessionState, scope: PanelScope<'_>, params: &Value) -> bool {
    let context = &params["requestContext"];
    if context["operation"] != "taskStatus" {
        return false;
    }
    if !connected(scope)
        || context["root"] != *session.thread_id()
        || context["epoch"] != scope.native.epoch
        || context["generation"] != scope.generation
    {
        return true;
    }
    let Some(read) = session
        .codex_task_status
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
    if !super::task_badge::owns(session, scope, &owner) {
        session.codex_task_status.read = None;
        return true;
    }
    if read.revision
        != *session
            .codex_task_status
            .revisions
            .get(&owner)
            .unwrap_or(&0)
    {
        session.codex_task_status.read = None;
        session.codex_task_status.dirty.insert(owner);
        return true;
    }
    if let Err(error) = pages::accept_page(session, scope, &params["response"]) {
        failed(session, scope, &error);
    }
    true
}

/// Wake for real pending work only; no timer remains after empty or unavailable process facts.
pub fn wait(session: &SessionState, scope: PanelScope<'_>, now: Instant) -> Option<Duration> {
    if !connected(scope) {
        return None;
    }
    let status = &session.codex_task_status;
    if let Some(read) = &status.read {
        return Some(if read.sent {
            (read.started + Duration::from_secs(20)).saturating_duration_since(now)
        } else {
            Duration::ZERO
        });
    }
    if session.codex_panel.snapshot.is_some() {
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

impl TaskStatus {
    pub(in crate::session) fn owner_revision(&self, owner: &str) -> u64 {
        self.revisions.get(owner).copied().unwrap_or(0)
    }
    /// Borrow failures for verified native owners.
    pub fn errors(&self) -> &HashMap<String, String> {
        &self.errors
    }
    /// Read owners whose cached processes are not yet authoritative.
    #[cfg(any(test, feature = "test-support"))]
    pub fn uncertain(&self) -> &HashSet<String> {
        &self.uncertain
    }
}
