//! Keep descendant activity outside the root transcript while retaining callback-independent state.

use serde_json::Value;

use super::codex_panel_pages::{parent, valid_id};
use super::codex_panel_state::{MAX_AGENTS, MAX_BYTES};
use super::state::AppState;

/// Observe child envelopes before the root filter; never adopt a child's identity as the root.
pub(super) fn event(app: &mut AppState, method: &str, params: &Value) -> bool {
    if !app.codex.enabled {
        return false;
    }
    super::codex_task_status::observed(app, method, params);
    let thread = &params["thread"];
    if matches!(method, "thread/started" | "thread/initialized") {
        if let Some(owner) = parent(thread) {
            if thread["id"] != app.thread_id
                && valid_id(&thread["id"]).is_ok()
                && super::task_badge::owns(app, &owner)
            {
                metadata(app, thread);
                return true;
            }
        }
        if thread["id"].as_str().is_some_and(|id| id != app.thread_id) {
            return true;
        }
    }
    let Some(id) = params["threadId"].as_str() else {
        return false;
    };
    if id == app.thread_id {
        if method == "item/completed" || method == "item/started" {
            report(app, &params["item"]);
        }
        return false;
    }
    if !known(app, id) {
        return true;
    }
    let index = app
        .codex
        .panel
        .agents
        .iter()
        .position(|agent| agent["id"] == id)
        .expect("known agent");
    match method {
        "thread/status/changed" => {
            if params["status"].to_string().len() > 16 * 1024 {
                return true;
            }
            set_field(app, index, "status", params["status"].clone());
            if params["status"]["type"] == "active" {
                set_field(app, index, "lastObservedTurn", Value::Null);
            }
            app.codex.panel.revision = app.codex.panel.revision.saturating_add(1);
        }
        "turn/started" | "turn/completed" => {
            if valid_id(&params["turn"]["id"]).is_err()
                || params["turn"]["status"]
                    .as_str()
                    .is_none_or(|status| status.len() > 128)
            {
                return true;
            }
            set_field(
                app,
                index,
                "lastObservedTurn",
                serde_json::json!({"id":params["turn"]["id"],"status":params["turn"]["status"]}),
            );
            app.codex.panel.revision = app.codex.panel.revision.saturating_add(1);
            app.codex.panel.refreshed = None;
        }
        "thread/closed" => {
            set_field(
                app,
                index,
                "status",
                serde_json::json!({"type":"notLoaded"}),
            );
            app.codex.panel.revision = app.codex.panel.revision.saturating_add(1);
        }
        "item/started" | "item/completed" => report(app, &params["item"]),
        _ => {}
    }
    super::codex_panel_activity::event(app, method, params);
    true
}

/// Retain bounded child report bodies in F2, including those embedded in root collaboration tools.
pub(super) fn report(app: &mut AppState, item: &Value) {
    if !app.codex.enabled || item["type"] != "collabAgentToolCall" {
        return;
    }
    let Some(states) = item["agentsStates"].as_object() else {
        return;
    };
    for (id, state) in states {
        if id.is_empty() || id.len() > 4096 || state.to_string().len() > 64 * 1024 {
            continue;
        }
        if !app.codex.panel.reports.contains_key(id) && app.codex.panel.reports.len() >= MAX_AGENTS
        {
            continue;
        }
        let bytes: usize = app
            .codex
            .panel
            .reports
            .values()
            .map(|value| value.to_string().len())
            .sum();
        if bytes + state.to_string().len() <= 1024 * 1024 {
            app.codex.panel.reports.insert(id.clone(), state.clone());
        }
    }
}

fn known(app: &AppState, id: &str) -> bool {
    app.codex.panel.agents.iter().any(|agent| agent["id"] == id)
}

fn metadata(app: &mut AppState, thread: &Value) {
    if !app.codex.panel.ownership.insert(
        &app.thread_id,
        app.codex.epoch,
        thread["id"].as_str().expect("validated child ID"),
    ) {
        app.codex.panel.error =
            Some("Verified task ownership reached its 512-ID bound; refresh F2".into());
        return;
    }
    if thread.to_string().len() > 64 * 1024 {
        app.codex.panel.refreshed = None;
        return;
    }
    let process_bytes: usize = app
        .codex
        .panel
        .processes
        .iter()
        .map(|row| row.data.to_string().len() + row.thread.len())
        .sum();
    let bytes: usize = app
        .codex
        .panel
        .agents
        .iter()
        .map(|agent| agent.to_string().len())
        .sum();
    if bytes + process_bytes + thread.to_string().len() > MAX_BYTES {
        return;
    }
    if let Some(index) = app
        .codex
        .panel
        .agents
        .iter()
        .position(|agent| agent["id"] == thread["id"])
    {
        app.codex.panel.agents[index] = thread.clone();
    } else if app.codex.panel.agents.len() < MAX_AGENTS {
        app.codex.panel.agents.push(thread.clone());
    }
    app.codex.panel.revision = app.codex.panel.revision.saturating_add(1);
    app.codex.panel.refreshed = None;
}

fn set_field(app: &mut AppState, index: usize, key: &str, value: Value) {
    let mut updated = app.codex.panel.agents[index].clone();
    updated[key] = value;
    let bytes: usize = app
        .codex
        .panel
        .agents
        .iter()
        .enumerate()
        .filter(|(row, _)| *row != index)
        .map(|(_, agent)| agent.to_string().len())
        .sum();
    let processes: usize = app
        .codex
        .panel
        .processes
        .iter()
        .map(|row| row.data.to_string().len() + row.thread.len())
        .sum();
    if bytes + processes + updated.to_string().len() <= MAX_BYTES {
        app.codex.panel.agents[index] = updated;
    } else {
        app.codex.panel.error =
            Some("Agent metadata exceeded8 MiB; refresh to read current state".into());
        app.codex.panel.refreshed = None;
    }
}
