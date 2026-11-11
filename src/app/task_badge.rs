//! Count only known active or waiting background work, independently of old panel rows.

use std::collections::HashSet;

use super::state::AppState;

/// Read current backend facts without treating completed rows or the root turn as tasks.
pub(super) fn count(app: &AppState) -> usize {
    if app.claude.enabled {
        return app.claude.tasks.active_count();
    }
    if !app.codex.enabled || app.codex.closed {
        return 0;
    }
    let mut agents = HashSet::new();
    for agent in &app.codex.panel.agents {
        let Some(id) = agent["id"].as_str().filter(|id| owns(app, id)) else {
            continue;
        };
        let active = agent["status"]["type"] == "active"
            && !matches!(
                agent["lastObservedTurn"]["status"].as_str(),
                Some("completed" | "failed" | "interrupted")
            );
        if active || agent["status"]["type"] == "systemError" {
            agents.insert(id);
        }
    }
    for callback in app.codex.callbacks.entries.values() {
        if let Some(id) = callback.params["threadId"]
            .as_str()
            .filter(|id| *id != app.thread_id && owns(app, id))
        {
            agents.insert(id);
        }
    }
    agents.len()
        + app
            .codex
            .panel
            .processes
            .iter()
            .filter(|row| {
                owns(app, &row.thread) && !app.codex.task_status.uncertain.contains(&row.thread)
            })
            .count()
}

/// Verify a cached owner still belongs to the current root after a fork or reconnect.
pub(super) fn owns(app: &AppState, owner: &str) -> bool {
    owner == app.thread_id
        || app
            .codex
            .panel
            .ownership
            .contains(&app.thread_id, app.codex.epoch, owner)
}
