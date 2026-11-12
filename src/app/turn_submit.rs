//! Turn submission and approval: starting, steering, interrupting turns, and responding to
//! pending tool-use approvals.

use std::time::{Duration, Instant};

use super::state::AppState;
use super::Role;
use crate::backend::{BackendClient, BackendKind, PromptOrigin, PromptPriority, RewindForkRequest};
use crate::grok_backend::GROK_PENDING_TURN_ID;
use crate::protocol_params::{extract_result_object, params_turn_steer};

pub(super) const CLAUDE_PENDING_TURN_ID: &str = "claude-turn-pending";

#[path = "turn_submit_history.rs"]
mod history;
use self::history::{
    apply_forked_thread_history, apply_grok_rewound_history, snapshot_grok_rewind_prefix,
};

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
    if text.trim().is_empty() || reject_rewind(client, app) {
        return;
    }
    let drop_turns = app
        .submitted_turn_count()
        .saturating_sub(rewind_history_idx);
    if drop_turns == 0 && client.kind() != BackendKind::Claude {
        submit_turn_text_with_history(client, app, text, true);
        return;
    }

    let grok_prefix = (client.kind() == BackendKind::Grok)
        .then(|| snapshot_grok_rewind_prefix(app, rewind_history_idx));
    let response = match client.fork_from_rewind(
        &app.session.thread_id(),
        RewindForkRequest {
            keep_turns: rewind_history_idx,
            drop_turns,
            before_turn_id: app.rewind_selected_turn_id().map(str::to_string),
            before_user_item: (client.kind() == BackendKind::Codex)
                .then(|| app.rewind_native_request())
                .flatten(),
        },
        Duration::from_secs(20),
    ) {
        Ok(response) => response,
        Err(err) => {
            app.set_status(format!("fork failed: {err}"));
            app.append_message(Role::System, format!("Could not edit message: {err}"));
            if client.kind() == BackendKind::Codex {
                app.view.viewport.auto_follow_bottom = true;
            }
            return;
        }
    };

    if client.kind() == BackendKind::Claude {
        queue_claude_edit(app, text);
        return;
    }

    let apply = match grok_prefix {
        Some(prefix) => apply_grok_rewound_history(app, &response, prefix),
        None => apply_forked_thread_history(app, &response),
    };
    if let Err(err) = apply {
        app.set_status(format!("fork failed: {err}"));
        return;
    }

    submit_edited_turn(client, app, text);
}

fn reject_rewind(client: &dyn BackendClient, app: &mut AppState) -> bool {
    if client.kind() == BackendKind::Codex && super::codex_queue::reject_switch(app) {
        return true;
    }

    if client.kind() == BackendKind::Claude
        && app.session.claude().phase() != super::claude_state::ClaudePhase::Ready
    {
        if !super::claude_edit::waiting(app) {
            super::claude_edit::unavailable(
                app,
                "Claude must be ready before editing an earlier prompt",
            );
        }
        return true;
    }
    false
}

fn queue_claude_edit(app: &mut AppState, text: String) {
    super::claude_edit::begin(app, text);
}

fn submit_edited_turn(client: &dyn BackendClient, app: &mut AppState, text: String) {
    app.clear_rewind_mode_state();
    app.view.viewport.selection = None;
    app.mark_user_turn_submitted();
    if start_new_turn(client, app, text.clone(), true, true, PromptOrigin::Human) {
        if client.kind() == BackendKind::Codex {
            app.push_input_history(&text);
        }
        app.clear_input();
    }
}

pub(super) fn submit_turn_text_with_history(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
) {
    submit_turn(
        client,
        app,
        text,
        record_input_history,
        false,
        PromptOrigin::Human,
    );
}

pub(super) fn submit_dequeued_turn_text(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
    origin: PromptOrigin,
) {
    submit_turn(client, app, text, record_input_history, true, origin);
}

fn submit_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
    force_start: bool,
    origin: PromptOrigin,
) {
    if text.trim().is_empty() {
        return;
    }
    if client.kind() == BackendKind::Codex
        && super::codex_lifecycle::reject_closed_prompt(app, &text, origin)
    {
        return;
    }

    if client.kind() == BackendKind::Claude
        && app.session.claude().phase() != crate::app::claude_state::ClaudePhase::Ready
    {
        if app.session.claude().phase() == super::claude_state::ClaudePhase::Closed {
            app.discard_pending_input_history(&text);
            app.set_input_text(&text);
            app.set_hint("Claude is closed; restart Carlos to send this draft");
            app.append_message(
                Role::System,
                "Claude is closed; restart Carlos to send this draft".to_string(),
            );
            return;
        }
        app.session
            .ralph_enqueue_prompt(text, record_input_history, origin);
        app.set_status("queued while Claude starts");
        return;
    }

    if !force_start
        && app.session.active_turn_id().is_some()
        && client.kind().queues_instead_of_steer()
    {
        start_new_turn(client, app, text, record_input_history, true, origin);
        return;
    }

    if let Some(turn_id) = app.session.active_turn_id().clone() {
        steer_existing_turn(client, app, &turn_id, &text, record_input_history, origin);
    } else {
        start_new_turn(client, app, text, record_input_history, force_start, origin);
    }
}

pub(super) use super::turn_approval::respond_to_pending_approval;

pub(super) use super::turn_interrupt::interrupt_active_turn;

fn steer_existing_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
    turn_id: &str,
    text: &str,
    record_input_history: bool,
    origin: PromptOrigin,
) {
    let params = params_turn_steer(&app.session.thread_id(), turn_id, text);
    match client.submit_prompt(
        "turn/steer",
        params,
        origin,
        if client.kind() == BackendKind::Claude && app.view.claude_queue {
            PromptPriority::Queue
        } else {
            PromptPriority::Steer
        },
        Duration::from_secs(10),
    ) {
        Ok(response) => {
            if client.kind().echoes_local_user_prompt() {
                let idx = app.append_message(Role::User, text.to_string());
                if record_input_history {
                    app.record_input_history(text, Some(idx));
                }
            }
            if client.kind() == BackendKind::Claude {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&response) {
                    super::claude_notifications::record_submitted_prompt(app, &parsed);
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
    force_start: bool,
    origin: PromptOrigin,
) -> bool {
    if client.kind() == BackendKind::Claude
        && app.session.claude().phase() != crate::app::claude_state::ClaudePhase::Ready
    {
        app.set_hint("Claude is starting; answer any startup question first");
        return false;
    }
    if queue_behind_pending(client, app, &text, record_input_history, force_start) {
        return true;
    }

    super::claude_edit::dispatching(app, &text, origin);
    let params = super::turn_settings::new_turn_params(client, app, &text);
    let response = client
        .submit_prompt(
            "turn/start",
            params,
            origin,
            if client.kind() == BackendKind::Claude
                && app.session.claude().activity().native_state()
                    == crate::session::claude::NativeState::Running
                && !app.view.claude_queue
            {
                PromptPriority::Steer
            } else {
                PromptPriority::Queue
            },
            Duration::from_secs(10),
        )
        .and_then(|response| extract_result_object(&response));
    match response {
        Ok(response) => {
            super::claude_edit::bind_submission(app, &text, origin, &response);
            mark_turn_sent(client, app, text, record_input_history, origin);
            if client.kind() == BackendKind::Claude {
                super::claude_notifications::record_submitted_prompt(app, &response);
            }
            true
        }
        Err(err) => {
            if record_input_history {
                app.discard_pending_input_history(&text);
            }
            if !super::claude_edit::send_failed(app, &text, &err.to_string()) {
                app.set_input_text(&text);
            }
            app.set_status(format!("send failed: {err}"));
            app.append_message(Role::System, format!("Could not send message: {err}"));
            false
        }
    }
}

fn queue_behind_pending(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: &str,
    record: bool,
    force: bool,
) -> bool {
    let now = Instant::now();
    if force || !client.kind().queues_follow_up_turns() || !app.has_ready_queued_turn_input(now) {
        return false;
    }
    app.promote_ready_continuation(now);
    app.queue_turn_input_with_history(text.to_string(), record);
    app.set_status(match client.kind() {
        BackendKind::Grok => "queued behind pending Grok turn",
        _ => "queued behind pending Claude turn",
    });
    true
}

fn mark_turn_sent(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: String,
    record_input_history: bool,
    origin: PromptOrigin,
) {
    if client.kind().echoes_local_user_prompt() {
        let role = if client.kind() == BackendKind::Claude && origin == PromptOrigin::Automatic {
            Role::System
        } else {
            Role::User
        };
        let idx = app.append_message(role, text.clone());
        if record_input_history {
            app.record_input_history(&text, Some(idx));
        }
        if app.session.active_turn_id().is_none() {
            app.mark_turn_started();
            app.session
                .set_active_turn_id(Some(pending_turn_id(client.kind()).to_string()));
        }
    }
    if client.kind() != BackendKind::Codex
        || !app.codex.enabled
        || app.codex.settings.legacy_overrides
    {
        app.mark_runtime_settings_applied();
    }
    app.set_status("sent turn");
}

fn pending_turn_id(kind: BackendKind) -> &'static str {
    match kind {
        BackendKind::Grok => GROK_PENDING_TURN_ID,
        _ => CLAUDE_PENDING_TURN_ID,
    }
}
