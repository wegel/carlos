//! Turn submission and approval: starting, steering, interrupting turns, and responding to
//! pending tool-use approvals.

use std::time::{Duration, Instant};

use super::state::{AppState, ApprovalChoice};
use super::Role;
use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::{params_turn_interrupt, params_turn_start, params_turn_steer};

pub(super) const CLAUDE_PENDING_TURN_ID: &str = "claude-turn-pending";

// --- Turn submission ---

pub(super) fn submit_turn_text(client: &dyn BackendClient, app: &mut AppState, text: String) {
    submit_turn_text_with_history(client, app, text, true);
}

pub(super) fn submit_turn_text_with_history(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
) {
    if text.trim().is_empty() {
        return;
    }

    if let Some(turn_id) = app.active_turn_id.clone() {
        steer_existing_turn(client, app, &turn_id, &text, record_input_history);
    } else {
        start_new_turn(client, app, text, record_input_history);
    }
}

// --- Approval response ---

pub(super) fn respond_to_pending_approval(
    client: &dyn BackendClient,
    app: &mut AppState,
    choice: ApprovalChoice,
) {
    let Some(pending) = app.approval.pending.clone() else {
        return;
    };
    let Some(result) = pending.response_for_choice(choice) else {
        app.set_status("approval choice not available");
        return;
    };

    match client.respond(&pending.request_id, result) {
        Ok(_) => {
            app.clear_pending_approval();
            let status = match choice {
                ApprovalChoice::Accept => "approval accepted",
                ApprovalChoice::AcceptForSession => "approval accepted for session",
                ApprovalChoice::Decline => "approval declined",
                ApprovalChoice::Cancel => "approval canceled",
            };
            app.set_status(status);
        }
        Err(err) => app.set_status(format!("approval reply failed: {err}")),
    }
}

// --- Turn interruption ---

pub(super) fn interrupt_active_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
) {
    let Some(turn_id) = app.active_turn_id.clone() else {
        return;
    };
    let params = params_turn_interrupt(&app.thread_id, &turn_id);
    match client.call("turn/interrupt", params, Duration::from_secs(10)) {
        Ok(_) => {
            app.append_turn_interrupted_marker();
            app.set_status("interrupt requested");
        }
        Err(e) => app.set_status(format!("{e}")),
    }
}

// --- Private helpers ---

fn steer_existing_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
    turn_id: &str,
    text: &str,
    record_input_history: bool,
) {
    let params = params_turn_steer(&app.thread_id, turn_id, text);
    match client.call("turn/steer", params, Duration::from_secs(10)) {
        Ok(_) => {
            if client.kind() == BackendKind::Claude {
                let idx = app.append_message(Role::User, text.to_string());
                if record_input_history {
                    app.record_input_history(text, Some(idx));
                }
            }
            app.set_status("sent steer");
        }
        Err(e) => app.set_status(format!("{e}")),
    }
}

fn start_new_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
) {
    let now = Instant::now();
    if client.kind() == BackendKind::Claude && app.has_ready_queued_turn_input(now) {
        app.promote_ready_continuation(now);
        app.queue_turn_input_with_history(text, record_input_history);
        app.set_status("queued behind pending Claude turn");
        return;
    }

    let (model, effort, summary) = app.next_turn_runtime_settings();
    let params = params_turn_start(
        &app.thread_id,
        &text,
        model.as_deref(),
        effort.as_deref(),
        summary.as_deref(),
    );
    match client.call("turn/start", params, Duration::from_secs(10)) {
        Ok(_) => {
            if client.kind() == BackendKind::Claude {
                let idx = app.append_message(Role::User, text.clone());
                if record_input_history {
                    app.record_input_history(&text, Some(idx));
                }
                app.mark_turn_started();
                app.active_turn_id = Some(CLAUDE_PENDING_TURN_ID.to_string());
            }
            app.mark_runtime_settings_applied();
            app.set_status("sent turn");
        }
        Err(e) => app.set_status(format!("{e}")),
    }
}
