//! Count only known active or waiting background work, independently of old panel rows.

use std::collections::HashSet;

use super::panel_scope::PanelScope;
use crate::session::state::SessionState;

/// Read current backend facts without treating completed rows or the root turn as tasks.
pub(crate) fn count(session: &SessionState, scope: PanelScope<'_>) -> usize {
    if session.claude().enabled() {
        return session.claude().tasks().active_count();
    }
    if !scope.native.enabled || scope.native.closed {
        return 0;
    }
    let mut agents = HashSet::new();
    for agent in &session.codex_panel.agents {
        let Some(id) = agent["id"].as_str().filter(|id| owns(session, scope, id)) else {
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
    for callback in session.codex_callbacks().iter().map(|(_, entry)| entry) {
        if let Some(id) = callback.params()["threadId"]
            .as_str()
            .filter(|id| *id != *session.thread_id() && owns(session, scope, id))
        {
            agents.insert(id);
        }
    }
    agents.len()
        + session
            .codex_panel
            .processes
            .iter()
            .filter(|row| {
                owns(session, scope, &row.thread)
                    && !session.codex_task_status.uncertain.contains(&row.thread)
            })
            .count()
}

/// Verify a cached owner still belongs to the current root after a fork or reconnect.
pub(crate) fn owns(session: &SessionState, scope: PanelScope<'_>, owner: &str) -> bool {
    owner == *session.thread_id()
        || session
            .codex_panel
            .ownership
            .contains(&session.thread_id(), scope.native.epoch, owner)
}
