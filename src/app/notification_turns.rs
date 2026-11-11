//! Updates turn state and stops the automatic Ralph loop after a failed turn.

use serde_json::Value;

use super::models::TurnStatus;
use super::state::{AppState, PendingApprovalRequest};

/// Track the active turn reported by the backend.
pub(super) fn handle_turn_started(app: &mut AppState, params: &serde_json::Map<String, Value>) {
    if let Some(id) = params
        .get("turn")
        .and_then(Value::as_object)
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
    {
        if !app.record_turn_started(id) {
            return;
        }
        app.context_usage = None;
        app.set_status("turn started");
    } else {
        app.mark_turn_started();
    }
}

/// Finish one turn and stop automatic Ralph work after a failure.
pub(super) fn handle_turn_completed(app: &mut AppState, params: &serde_json::Map<String, Value>) {
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
    if !app.record_turn_completed(turn_id, turn_status) {
        return;
    }
    let keep_pending = app
        .approval
        .pending
        .as_ref()
        .map(PendingApprovalRequest::persists_after_turn_completed)
        .unwrap_or(false);
    if !keep_pending {
        app.clear_pending_approval();
    }
    let interrupted = turn_status == TurnStatus::Interrupted;
    if interrupted {
        app.append_turn_interrupted_marker();
        app.set_status("turn interrupted");
    } else if turn_status == TurnStatus::Failed {
        app.ralph_runtime.stop_after_failure();
        app.turn_start_message_idx = None;
        app.set_status("turn failed");
    } else {
        app.set_status("turn completed");
    }
    if turn_status != TurnStatus::Failed {
        app.handle_ralph_turn_completed(interrupted);
    }
    if let Err(e) = app.apply_pending_ralph_toggle() {
        app.set_status(format!("ralph: {e}"));
    }
}
