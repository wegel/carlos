//! Fence a terminal Codex transport failure without losing the person's draft.

use serde_json::{Map, Value};

use super::models::{Role, TurnStatus};
use super::state::AppState;
use crate::backend::PromptOrigin;
use crate::session::approval_state::ApprovalQueue;

/// Close the UI once; later frames cannot reopen a turn or an approval.
pub(super) fn close(app: &mut AppState, params: &Map<String, Value>) {
    close_with_guidance(app, params, "");
}

/// Keep retry keys in the view while recording the backend failure as a fact.
pub(super) fn close_with_guidance(app: &mut AppState, params: &Map<String, Value>, guidance: &str) {
    if app.codex.connection.closed_generation == Some(app.codex.connection.generation) {
        return;
    }
    app.codex.connection.closed_generation = Some(app.codex.connection.generation);
    app.codex.connection.rebuilding = false;
    app.codex.closed = true;
    app.clear_rate_limits();
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
    let recovery = app.session.take_codex_queue_recovery();
    super::codex_queue_receive::recover(app, recovery);
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
    app.append_message(Role::System, format!("{label}: {message}{guidance}. F5 reconnects; drafts are kept. Unconfirmed input will not be resent."));
    app.set_status(format!("{label}: {message}"));
    app.set_hint(format!("{label}: {message}{guidance}. F5 reconnects"));
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
    app.session.close_codex_goals();
    app.view.codex_goals.confirm = None;
    app.view.codex_goals.error = None;
    app.codex.panel.outgoing.clear();
    app.codex.panel.snapshot = None;
    app.codex.panel.stop = None;
    app.codex.panel.error = Some("Codex closed; pending panel work was not confirmed".into());
    if let Some(activity) = app.codex.panel.activity.as_mut() {
        activity.request = None;
    }
    app.session.close_codex_queue();
    if app.codex.connection.socket.is_some() {
        let children = app
            .session
            .codex_callbacks()
            .iter()
            .map(|(_, entry)| entry)
            .filter(|entry| entry.params()["threadId"] != *app.session.thread_id())
            .count();
        app.codex.connection.unreplayed_children =
            app.codex.connection.unreplayed_children.max(children);
    }
    app.session.clear_codex_callbacks();
    app.view.codex_callbacks = Default::default();
    let mut retired = app.session.clear_approval_queue(ApprovalQueue::Codex);
    retired.extend(app.session.retire_active_approval());
    app.drop_retired_approval_hints(retired);
    app.session.clear_turn_boundary();
}
