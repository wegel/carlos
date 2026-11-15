//! Keep descendant activity outside the root transcript while retaining callback-independent state.

use serde_json::Value;

use super::panel_pages::{parent, valid_id};
use super::panel_scope::PanelScope;
use super::panel_state::{MAX_AGENTS, MAX_BYTES};
use crate::session::state::SessionState;

/// Observe child envelopes before the root filter; never adopt a child's identity as the root.
pub fn event(
    session: &mut SessionState,
    scope: PanelScope<'_>,
    method: &str,
    params: &Value,
) -> bool {
    if !scope.native.enabled {
        return false;
    }
    super::task_status::observed(session, scope, method, params);
    if matches!(method, "thread/started" | "thread/initialized")
        && accept_thread(session, scope, &params["thread"])
    {
        return true;
    }
    let Some(id) = params["threadId"].as_str() else {
        return false;
    };
    if id == *session.thread_id() {
        if method == "item/completed" || method == "item/started" {
            report(session, &params["item"]);
        }
        return false;
    }
    super::panel_activity::event(session, method, params);
    if !known(session, id) {
        return true;
    }
    let index = session
        .codex_panel
        .agents
        .iter()
        .position(|agent| agent["id"] == id)
        .expect("known agent");
    if !apply_child(session, index, method, params) {
        return true;
    }
    true
}

/// Retain bounded child report bodies in F2, including those embedded in root collaboration tools.
pub fn report(session: &mut SessionState, item: &Value) {
    if !session.codex_scope().enabled() || item["type"] != "collabAgentToolCall" {
        return;
    }
    let Some(states) = item["agentsStates"].as_object() else {
        return;
    };
    for (id, state) in states {
        if id.is_empty() || id.len() > 4096 || state.to_string().len() > 64 * 1024 {
            continue;
        }
        if !session.codex_panel.reports.contains_key(id)
            && session.codex_panel.reports.len() >= MAX_AGENTS
        {
            continue;
        }
        let bytes: usize = session
            .codex_panel
            .reports
            .values()
            .map(|value| value.to_string().len())
            .sum();
        if bytes + state.to_string().len() <= 1024 * 1024 {
            session.task_metadata_changed();
            session
                .codex_panel
                .reports
                .insert(id.clone(), state.clone());
        }
    }
}

fn known(session: &SessionState, id: &str) -> bool {
    session
        .codex_panel
        .agents
        .iter()
        .any(|agent| agent["id"] == id)
}

fn metadata(session: &mut SessionState, scope: PanelScope<'_>, thread: &Value) {
    session.task_metadata_changed();
    if !session.codex_panel.ownership.insert(
        &session.thread_id,
        scope.native.epoch,
        thread["id"].as_str().expect("validated child ID"),
    ) {
        session.codex_panel.error =
            Some("Verified task ownership reached its 512-ID bound; refresh F2".into());
        return;
    }
    if thread.to_string().len() > 64 * 1024 {
        session.codex_panel.refreshed = None;
        return;
    }
    let process_bytes: usize = session
        .codex_panel
        .processes
        .iter()
        .map(|row| row.data.to_string().len() + row.thread.len())
        .sum();
    let bytes: usize = session
        .codex_panel
        .agents
        .iter()
        .map(|agent| agent.to_string().len())
        .sum();
    if bytes + process_bytes + thread.to_string().len() > MAX_BYTES {
        return;
    }
    if let Some(index) = session
        .codex_panel
        .agents
        .iter()
        .position(|agent| agent["id"] == thread["id"])
    {
        session.codex_panel.agents[index] = thread.clone();
    } else if session.codex_panel.agents.len() < MAX_AGENTS {
        session.codex_panel.agents.push(thread.clone());
    }
    session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
    session.codex_panel.refreshed = None;
}

fn set_field(session: &mut SessionState, index: usize, key: &str, value: Value) {
    session.task_metadata_changed();
    let mut updated = session.codex_panel.agents[index].clone();
    updated[key] = value;
    let bytes: usize = session
        .codex_panel
        .agents
        .iter()
        .enumerate()
        .filter(|(row, _)| *row != index)
        .map(|(_, agent)| agent.to_string().len())
        .sum();
    let processes: usize = session
        .codex_panel
        .processes
        .iter()
        .map(|row| row.data.to_string().len() + row.thread.len())
        .sum();
    if bytes + processes + updated.to_string().len() <= MAX_BYTES {
        session.codex_panel.agents[index] = updated;
    } else {
        session.codex_panel.error =
            Some("Agent metadata exceeded8 MiB; refresh to read current state".into());
        session.codex_panel.refreshed = None;
    }
}

fn accept_thread(session: &mut SessionState, scope: PanelScope<'_>, thread: &Value) -> bool {
    if let Some(owner) = parent(thread) {
        if thread["id"] != *session.thread_id()
            && valid_id(&thread["id"]).is_ok()
            && super::task_badge::owns(session, scope, &owner)
        {
            metadata(session, scope, thread);
            return true;
        }
    }
    thread["id"]
        .as_str()
        .is_some_and(|id| id != *session.thread_id())
}

fn apply_child(session: &mut SessionState, index: usize, method: &str, params: &Value) -> bool {
    match method {
        "thread/status/changed" => {
            if params["status"].to_string().len() > 16 * 1024 {
                return false;
            }
            set_field(session, index, "status", params["status"].clone());
            if params["status"]["type"] == "active" {
                set_field(session, index, "lastObservedTurn", Value::Null);
            }
            session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
        }
        "turn/started" | "turn/completed" => {
            if valid_id(&params["turn"]["id"]).is_err()
                || params["turn"]["status"]
                    .as_str()
                    .is_none_or(|status| status.len() > 128)
            {
                return false;
            }
            set_field(
                session,
                index,
                "lastObservedTurn",
                serde_json::json!({"id":params["turn"]["id"],"status":params["turn"]["status"]}),
            );
            session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
            session.codex_panel.refreshed = None;
        }
        "thread/closed" => {
            set_field(
                session,
                index,
                "status",
                serde_json::json!({"type":"notLoaded"}),
            );
            session.codex_panel.revision = session.codex_panel.revision.saturating_add(1);
        }
        "item/started" | "item/completed" => report(session, &params["item"]),
        _ => {}
    }
    true
}
