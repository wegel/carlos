//! Updates turn state and stops the automatic Ralph loop after a failed turn.

use serde_json::Value;

use super::common_update::CommonUpdate;
use super::state::SessionState;
use crate::session::models::TurnStatus;

/// Track the active turn reported by the backend.
pub(in crate::session) fn handle_turn_started(
    session: &mut SessionState,
    params: &serde_json::Map<String, Value>,
) {
    if let Some(id) = params
        .get("turn")
        .and_then(Value::as_object)
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
    {
        if !session.record_turn_started(id) {
            return;
        }
        if session.codex_scope().enabled() {
            super::codex::settings::started(session, id);
        }
        session.set_context_usage(None);
        session.set_status("turn started");
    } else {
        session.mark_turn_started();
    }
}

/// Finish one turn and stop automatic Ralph work after a failure.
pub(in crate::session) fn handle_turn_completed(
    session: &mut SessionState,
    update: &mut CommonUpdate,
    params: &serde_json::Map<String, Value>,
) {
    let Some(turn) = params.get("turn").and_then(Value::as_object) else {
        return;
    };
    let Some(turn_id) = turn.get("id").and_then(Value::as_str) else {
        return;
    };
    let Some(turn_status) = turn
        .get("status")
        .and_then(Value::as_str)
        .and_then(TurnStatus::from_wire)
    else {
        return;
    };
    if !session.record_turn_completed(turn_id, turn_status) {
        return;
    }
    if session.codex_scope().enabled() {
        session.invalidate_codex_queue();
    }
    if session.events.backend == carlos_proto::session::Backend::Grok {
        session.clear_grok_permissions();
    }
    let keep_codex = session.approval().pending().is_some_and(|pending| {
        session
            .codex_callbacks()
            .get(&pending.facts().request_id)
            .is_some()
    });
    let keep_pending = keep_codex
        || session
            .approval()
            .pending()
            .map(|pending| pending.facts().persists_after_turn_completed())
            .unwrap_or(false);
    if !keep_pending {
        update.cleared = Some(session.clear_pending_session_approval());
    }
    let interrupted = turn_status == TurnStatus::Interrupted;
    if interrupted {
        session.append_turn_interrupted_marker();
        session.set_status("turn interrupted");
    } else if turn_status == TurnStatus::Failed {
        session.ralph_stop_after_failure();
        session.clear_turn_boundary();
        session.set_status("turn failed");
    } else {
        session.set_status("turn completed");
    }
    if turn_status != TurnStatus::Failed {
        session.handle_ralph_turn_completed(interrupted);
    }
    if session.ralph_runtime().toggle_pending() {
        update.action = Some(super::actions::ServerRequestAction::ApplyRalphToggle);
    }
}
