//! Translates raw SDK messages at the UI boundary and reads native models and context.

use serde_json::{Map, Value};

use super::claude_callbacks::{cancel_callback, receive_callback};
use super::notifications::{handle_server_message_line, ServerRequestAction};
use super::state::AppState;

/// Attach an accepted native prompt UUID to its visible user message.
pub(super) fn record_submitted_prompt(app: &mut AppState, response: &Value) {
    if let Some(id) = response["result"]["turn"]["id"].as_str() {
        if app.active_turn_id.as_deref() == Some(super::turn_submit::CLAUDE_PENDING_TURN_ID)
            || app.active_turn_id.is_none()
        {
            app.record_turn_started(id);
        }
        super::claude_prompts::attach_echo(app, id, response["result"]["inputKind"].as_str());
        if let Some(index) = app.messages.len().checked_sub(1) {
            app.input_history.record_turn_id(index, id);
        }
        super::claude_edit::accepted(app, id);
    }
}

/// Handle Claude-only frames before the shared app-server message parser.
pub(super) fn handle(
    app: &mut AppState,
    method: &str,
    params: &Map<String, Value>,
) -> Option<ServerRequestAction> {
    app.claude.enabled = true;
    let generation = params.get("generation").and_then(Value::as_u64);
    if method == "claude/sessionChanged" {
        if let (Some(generation), Some(id)) =
            (generation, params.get("sessionId").and_then(Value::as_str))
        {
            if generation > app.claude.generation {
                super::claude_session_history::switch_session(app, generation, id);
            }
        }
        return None;
    }
    if generation.is_some_and(|generation| generation != app.claude.generation) {
        return None;
    }
    match method {
        "claude/sessionStartFailed" => {
            fail_session(app);
            let message = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Claude could not start the edited branch");
            super::claude_edit::failed(app, message);
        }
        "claude/forkFailed" => {
            let message = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Could not fork Claude session");
            super::claude_edit::failed(app, message);
        }
        "claude/promptSubmitted" => {
            return accept_prompt(app, params);
        }
        "claude/promptRejected" => {
            if let Some(id) = params["uuid"].as_str() {
                app.claude.rejected(id);
                app.claude.prompts.entries.remove(id);
            }
        }
        "claude/controller" => return handle_controller(app, params),
        _ => {}
    }
    None
}

fn handle_frame(
    app: &mut AppState,
    frame: &Value,
    request: Option<&str>,
    context: &Value,
) -> Option<ServerRequestAction> {
    if app.claude.phase == super::claude_state::ClaudePhase::Closed
        && (matches!(frame["type"].as_str(), Some("callback" | "reply")) || frame["event"] == "sdk")
    {
        return None;
    }
    match frame["type"].as_str() {
        Some("callback") => receive_callback(app, frame),
        Some("reply") => {
            return super::claude_control_replies::receive(app, frame, request, context)
        }
        Some("event") => return handle_event(app, frame),
        _ => {}
    }
    None
}

fn handle_event(app: &mut AppState, frame: &Value) -> Option<ServerRequestAction> {
    match frame["event"].as_str() {
        Some("sdk") => return super::claude_sdk_events::receive(app, &frame["message"]),
        Some("stderr") => {
            if let Some(text) = frame["text"].as_str() {
                super::claude_notices::show(app, format!("stderr:{text}"), text.to_string());
            }
        }
        Some("callback_cancelled") => cancel_callback(app, &frame["callbackId"]),
        Some("error" | "transport_error") => {
            if frame["error"]["code"] == "unsupported_dialog" {
                app.append_message(
                    super::Role::System,
                    format!(
                        "{} (request {}, kind {})",
                        frame["error"]["message"]
                            .as_str()
                            .unwrap_or("Unsupported Claude dialog"),
                        super::claude_question_state::safe_label(
                            frame["callbackId"].as_str().unwrap_or("unknown")
                        ),
                        super::claude_question_state::safe_label(
                            frame["dialogKind"].as_str().unwrap_or("unknown")
                        )
                    ),
                );
            } else {
                show_error(app, &frame["error"]);
                fail_session(app);
            }
        }
        Some("closed") => {
            let reason = frame["reason"].as_str().unwrap_or("unknown");
            fail_session(app);
            app.set_status(format!("Claude closed: {reason}"));
            app.append_message(super::Role::System, format!("Claude closed: {reason}"));
        }
        _ => {}
    }
    None
}

/// Prefer native error details and retain a useful message when its error list is empty.
pub(super) fn result_error(message: &Value) -> String {
    message["errors"]
        .as_array()
        .map(|errors| {
            errors
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|error| !error.is_empty())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|error| !error.is_empty())
        .or_else(|| {
            message["result"]
                .as_str()
                .map(str::trim)
                .filter(|error| !error.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "Claude turn failed".to_string())
}

/// Close the UI query and recover unsent drafts after a terminal helper failure.
pub(super) fn fail_session(app: &mut AppState) {
    super::claude_edit::recover_sending(app);
    super::claude_session_history::restore_edit(app);
    app.claude.phase = crate::app::claude_state::ClaudePhase::Closed;
    if super::claude_edit::waiting(app) {
        app.claude.history.edit = Some(super::claude_edit::EditState::Failed);
    }
    app.claude.questions.clear();
    app.clear_pending_approval();
    for line in app.claude.fail_pending() {
        let _ = handle_server_message_line(app, &line);
    }
    app.claude.activity = Default::default();
    app.claude.tasks.live.clear();
    app.ralph_runtime.stop_after_failure();
    super::claude_session_history::recover_queued(app);
}

pub(super) fn show_error(app: &mut AppState, error: &Value) {
    let message = error["message"].as_str().unwrap_or("Claude SDK error");
    app.append_message(super::Role::System, message.to_string());
    app.set_status(message);
}

fn handle_controller(
    app: &mut AppState,
    params: &Map<String, Value>,
) -> Option<ServerRequestAction> {
    let frame = &params["frame"];
    let epoch = params
        .get("epoch")
        .and_then(Value::as_u64)
        .unwrap_or(app.claude.epoch);
    let process = frame["type"] == "callback"
        || frame["event"] == "callback_cancelled"
        || matches!(
            params.get("requestMethod").and_then(Value::as_str),
            Some("start" | "capabilities" | "task.stop" | "usage.optional" | "context")
        )
        || crate::claude_backend::is_process_message(&frame["message"]);
    if epoch < app.claude.epoch && !process {
        return None;
    }
    if frame["event"] == "sdk"
        && frame["message"]["type"] == "conversation_reset"
        && !crate::claude_backend::is_child_message(&frame["message"])
    {
        app.claude.epoch = epoch;
    }
    handle_frame(
        app,
        &params["frame"],
        params.get("requestMethod").and_then(Value::as_str),
        params.get("requestContext").unwrap_or(&Value::Null),
    )
}

fn accept_prompt(app: &mut AppState, params: &Map<String, Value>) -> Option<ServerRequestAction> {
    if let Some(id) = params["uuid"].as_str() {
        app.claude.submitted(id);
        if let Err(error) = app.claude.prompts.register(&Value::Object(params.clone())) {
            app.append_message(super::Role::System, error.to_string());
            fail_session(app);
            return Some(ServerRequestAction::ClaudeControl {
                method: "shutdown".to_string(),
                params: serde_json::json!({}),
            });
        }
    }
    None
}
