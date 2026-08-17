//! Turn submission and approval: starting, steering, interrupting turns, and responding to
//! pending tool-use approvals.

use std::time::{Duration, Instant};

use anyhow::Context;

use super::notifications::load_history_from_start_or_resume;
use super::state::{AppState, ApprovalChoice};
use super::Role;
use crate::backend::{BackendClient, BackendKind, RewindForkRequest};
use crate::grok_backend::GROK_PENDING_TURN_ID;
use crate::protocol_params::{
    params_turn_interrupt, params_turn_start, params_turn_steer,
    parse_thread_id_from_start_or_resume,
};

pub(super) const CLAUDE_PENDING_TURN_ID: &str = "claude-turn-pending";

// --- Turn submission ---

pub(super) fn submit_turn_text(client: &dyn BackendClient, app: &mut AppState, text: String) {
    submit_turn_text_with_history(client, app, text, true);
}

pub(super) fn submit_rewind_turn_text(
    client: &dyn BackendClient,
    app: &mut AppState,
    rewind_history_idx: usize,
    text: String,
) {
    if text.trim().is_empty() {
        return;
    }

    let total_turns = app.submitted_turn_count();
    let drop_turns = total_turns.saturating_sub(rewind_history_idx);
    if drop_turns == 0 {
        submit_turn_text_with_history(client, app, text, true);
        return;
    }

    let grok_prefix = (client.kind() == BackendKind::Grok)
        .then(|| snapshot_grok_rewind_prefix(app, rewind_history_idx));

    let response = match client.fork_from_rewind(
        &app.thread_id,
        RewindForkRequest {
            keep_turns: rewind_history_idx,
            drop_turns,
        },
        Duration::from_secs(20),
    ) {
        Ok(response) => response,
        Err(err) => {
            app.set_status(format!("fork failed: {err}"));
            return;
        }
    };

    let apply = match grok_prefix {
        Some(prefix) => apply_grok_rewound_history(app, &response, prefix),
        None => apply_forked_thread_history(app, &response),
    };
    if let Err(err) = apply {
        app.set_status(format!("fork failed: {err}"));
        return;
    }

    app.clear_rewind_mode_state();
    app.push_input_history(&text);
    app.clear_input();
    app.viewport.selection = None;
    app.mark_user_turn_submitted();
    start_new_turn(client, app, text, true);
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

    if app.active_turn_id.is_some() && client.kind().queues_instead_of_steer() {
        app.queue_turn_input_with_history(text, record_input_history);
        app.set_status("queued behind pending Grok turn");
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

pub(super) fn interrupt_active_turn(client: &dyn BackendClient, app: &mut AppState) {
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

fn snapshot_grok_rewind_prefix(
    app: &AppState,
    rewind_history_idx: usize,
) -> Vec<crate::app::models::Message> {
    let cut = app
        .rewind_selected_message_idx()
        .or_else(|| nth_user_message_idx(app, rewind_history_idx))
        .unwrap_or(app.messages.len());
    let cut = cut.min(app.messages.len());
    app.messages[..cut].to_vec()
}

fn nth_user_message_idx(app: &AppState, rewind_history_idx: usize) -> Option<usize> {
    app.messages
        .iter()
        .enumerate()
        .filter(|(_, message)| message.role == Role::User)
        .nth(rewind_history_idx)
        .map(|(idx, _)| idx)
}

fn apply_grok_rewound_history(
    app: &mut AppState,
    response_line: &str,
    prefix: Vec<crate::app::models::Message>,
) -> anyhow::Result<()> {
    let thread_id =
        parse_thread_id_from_start_or_resume(response_line).context("missing rewound thread id")?;
    app.reset_for_forked_thread(thread_id);
    for message in prefix {
        let record_user = message.role == Role::User && !message.text.is_empty();
        let text = message.text.clone();
        app.messages.push(message);
        let idx = app.messages.len() - 1;
        if record_user {
            app.record_input_history(&text, Some(idx));
        }
        app.mark_transcript_dirty_from(idx);
    }
    Ok(())
}

fn apply_forked_thread_history(app: &mut AppState, response_line: &str) -> anyhow::Result<()> {
    let thread_id =
        parse_thread_id_from_start_or_resume(response_line).context("missing forked thread id")?;
    app.reset_for_forked_thread(thread_id);
    load_history_from_start_or_resume(app, response_line)
        .context("failed to load forked thread history")?;
    Ok(())
}

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
            if client.kind().echoes_local_user_prompt() {
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
    if client.kind().queues_follow_up_turns() && app.has_ready_queued_turn_input(now) {
        app.promote_ready_continuation(now);
        app.queue_turn_input_with_history(text, record_input_history);
        app.set_status(match client.kind() {
            BackendKind::Grok => "queued behind pending Grok turn",
            _ => "queued behind pending Claude turn",
        });
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
            if client.kind().echoes_local_user_prompt() {
                let idx = app.append_message(Role::User, text.clone());
                if record_input_history {
                    app.record_input_history(&text, Some(idx));
                }
                app.mark_turn_started();
                app.active_turn_id = Some(pending_turn_id(client.kind()).to_string());
            }
            app.mark_runtime_settings_applied();
            app.set_status("sent turn");
        }
        Err(e) => app.set_status(format!("{e}")),
    }
}

fn pending_turn_id(kind: BackendKind) -> &'static str {
    match kind {
        BackendKind::Grok => GROK_PENDING_TURN_ID,
        _ => CLAUDE_PENDING_TURN_ID,
    }
}
