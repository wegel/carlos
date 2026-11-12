//! Fence a terminal Codex transport failure without losing the person's draft.

use serde_json::{Map, Value};

use super::models::{Role, TurnStatus};
use super::state::AppState;
use crate::backend::PromptOrigin;
use crate::session::approval_state::ApprovalQueue;

/// Close the UI once; later frames cannot reopen a turn or an approval.
pub(super) fn close(app: &mut AppState, params: &Map<String, Value>) {
    if app.codex.connection.closed_generation == Some(app.codex.connection.generation) {
        return;
    }
    app.codex.connection.closed_generation = Some(app.codex.connection.generation);
    app.codex.connection.rebuilding = false;
    app.codex.closed = true;
    app.rate_limits.clear();
    app.codex.connection.streams = Default::default();
    app.codex.connection.runtime_read = None;
    app.codex.connection.runtime_refresh = false;
    if let Some(turn_id) = app.session.active_turn_id().clone() {
        if app.codex.connection.socket.is_none() {
            app.record_turn_completed(&turn_id, TurnStatus::Failed);
        } else {
            app.session.set_active_turn_id(None);
        }
    }
    app.session.ralph_stop_after_failure();
    super::codex_settings::closed(app);
    if let Some(operation) = app.codex.queue.operation.take() {
        super::codex_queue_receive::recover(app, operation.submitted);
    }
    clear_pending_controls(app);
    let message = params
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Codex closed");
    let message = super::claude_question_state::safe_text(message);
    let label = if app.codex.connection.socket.is_some() {
        "Codex connection lost"
    } else {
        "Codex stopped"
    };
    app.append_message(Role::System, format!("{label}: {message}. F5 reconnects; drafts are kept. Unconfirmed input will not be resent."));
    app.set_status(format!("{label}: {message}. F5 reconnects"));
}

/// Keep human input recoverable and discard automatic work after terminal closure.
pub(super) fn reject_closed_prompt(app: &mut AppState, text: &str, origin: PromptOrigin) -> bool {
    if !app.codex.closed {
        return false;
    }
    app.session.ralph_stop_after_failure();
    if origin == PromptOrigin::Human {
        app.set_input_text(text);
        app.append_message(
            Role::System,
            "Could not send message: Codex is closed; F5 reconnects; draft kept",
        );
    }
    app.set_hint("Codex is closed; F5 reconnects; draft kept");
    true
}

fn clear_pending_controls(app: &mut AppState) {
    app.codex.goals.read = None;
    app.codex.goals.mutation = None;
    app.codex.goals.confirm = None;
    app.codex.goals.notice.clear();
    app.codex.goals.error =
        Some("Previous connection closed before the goal change was confirmed. Draft kept; no automatic retry".into());
    app.codex.panel.outgoing.clear();
    app.codex.panel.snapshot = None;
    app.codex.panel.stop = None;
    app.codex.panel.error = Some("Codex closed; pending panel work was not confirmed".into());
    if let Some(activity) = app.codex.panel.activity.as_mut() {
        activity.request = None;
    }
    app.codex.queue.read = None;
    app.codex.queue.known = false;
    if app.codex.connection.socket.is_some() {
        let children = app
            .codex
            .callbacks
            .entries
            .values()
            .filter(|entry| entry.params["threadId"] != *app.session.thread_id())
            .count();
        app.codex.connection.unreplayed_children =
            app.codex.connection.unreplayed_children.max(children);
    }
    app.codex.callbacks = Default::default();
    app.session.clear_approval_queue(ApprovalQueue::Codex);
    if let Some(id) = app.session.approval().pending().and_then(|p| p.entry_id()) {
        app.session.retire_approval(id);
    }
    app.retain_live_approval_hints();
    app.session.clear_turn_boundary();
}
