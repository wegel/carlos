//! Applies normalized Claude facts and retains native prompt ownership for the app.

use serde_json::{Map, Value};

use super::claude_callbacks::{cancel_callback, receive_callback};
use super::models::TurnStatus;
use super::notifications::ServerRequestAction;
use super::state::AppState;

/// Distinguishes a normalized Claude fact from another backend's shared notification.
pub(super) enum ClaudeRoute {
    Continue,
    Handled(Option<ServerRequestAction>),
}

/// Fence common Claude messages and apply the explicitly named backend facts.
pub(super) fn route(app: &mut AppState, method: &str, params: &Map<String, Value>) -> ClaudeRoute {
    if method.starts_with("claude/") && named_fact(method) {
        return ClaudeRoute::Handled(handle(app, method, params));
    }
    if !app.claude.enabled {
        return ClaudeRoute::Continue;
    }
    let obsolete_generation = params
        .get("generation")
        .and_then(Value::as_u64)
        .is_some_and(|generation| generation != app.claude.generation);
    let obsolete_epoch = params
        .get("epoch")
        .and_then(Value::as_u64)
        .is_some_and(|epoch| epoch < app.claude.epoch);
    if obsolete_generation || (obsolete_epoch && !params.contains_key("claudeContext")) {
        return ClaudeRoute::Handled(None);
    }
    if app.claude.phase == super::claude_state::ClaudePhase::Closed
        && params.contains_key("generation")
    {
        return ClaudeRoute::Handled(None);
    }
    if method == "thread/tokenUsage/updated" && params.contains_key("claudeContext") {
        super::claude_context::receive(
            app,
            &serde_json::json!({"ok":true,"result":params["claudeContext"]}),
            params.get("requestContext").unwrap_or(&Value::Null),
        );
        return ClaudeRoute::Handled(None);
    }
    if method == "turn/completed" && params.contains_key("generation") {
        if let Some(id) = params.get("turn").and_then(|turn| turn["id"].as_str()) {
            app.claude.prompts.entries.remove(id);
        }
    }
    if params.get("history") == Some(&Value::Bool(true)) {
        app.with_historical_tools(|app| {
            super::notification_items::handle_item_notification(app, method, params);
        });
        return ClaudeRoute::Handled(None);
    }
    ClaudeRoute::Continue
}

fn named_fact(method: &str) -> bool {
    matches!(
        method,
        "claude/sessionChanged"
            | "claude/sessionStartFailed"
            | "claude/forkFailed"
            | "claude/promptSubmitted"
            | "claude/promptRejected"
            | "claude/controlReply"
            | "claude/callback"
            | "claude/callbackCancelled"
            | "claude/historyRecord"
            | "claude/historyFactPart"
            | "claude/historyPrompt"
            | "claude/historyCompacted"
            | "claude/stderr"
            | "claude/error"
            | "claude/closed"
            | "claude/quota"
            | "claude/browserComplete"
            | "claude/task"
            | "claude/conversationReset"
            | "claude/activity"
            | "claude/status"
            | "claude/thinking"
            | "claude/result"
            | "claude/usageReport"
            | "claude/notice"
            | "claude/initialized"
            | "claude/promptsConsumed"
            | "claude/promptsRetired"
    )
}

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

/// Handle named Claude facts before the shared app-server message parser.
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
    if !can_apply(app, method, params) {
        return None;
    }
    match method {
        "claude/sessionStartFailed" | "claude/forkFailed" => startup_failure(app, method, params),
        "claude/promptSubmitted" => {
            return accept_prompt(app, params);
        }
        "claude/promptRejected" => {
            if let Some(id) = params["uuid"].as_str() {
                app.claude.prompts.entries.remove(id);
            }
        }
        "claude/controlReply" => {
            return super::claude_control_replies::receive(
                app,
                &Value::Object(params.clone()),
                params.get("requestMethod").and_then(Value::as_str),
                params.get("requestContext").unwrap_or(&Value::Null),
            )
        }
        "claude/callback" => receive_callback(app, &Value::Object(params.clone())),
        "claude/callbackCancelled" => cancel_callback(app, &params["callbackId"]),
        "claude/historyFactPart" => return receive_history_part(app, params),
        "claude/historyRecord" => app.claude.history.records.push(params["record"].clone()),
        "claude/historyPrompt" => {
            super::claude_session_history::append_prompt(app, &Value::Object(params.clone()))
        }
        "claude/historyCompacted" => {
            app.append_message(super::Role::System, "Conversation compacted".to_string());
        }
        "claude/stderr" | "claude/error" | "claude/closed" => {
            return handle_event(app, method, &Value::Object(params.clone()))
        }
        _ => return super::claude_sdk_events::receive(app, method, &Value::Object(params.clone())),
    }
    None
}

fn handle_event(app: &mut AppState, method: &str, frame: &Value) -> Option<ServerRequestAction> {
    match method {
        "claude/stderr" => {
            if let Some(text) = frame["text"].as_str() {
                super::claude_notices::show(app, format!("stderr:{text}"), text.to_string());
            }
        }
        "claude/error" => {
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
        "claude/closed" => {
            let reason = frame["reason"].as_str().unwrap_or("unknown");
            fail_session(app);
            app.set_status(format!("Claude closed: {reason}"));
            app.append_message(super::Role::System, format!("Claude closed: {reason}"));
        }
        _ => {}
    }
    None
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
    fail_accepted_prompts(app);
    app.claude.activity = Default::default();
    app.claude.tasks.live.clear();
    app.ralph_runtime.stop_after_failure();
    super::claude_session_history::recover_queued(app);
}

fn fail_accepted_prompts(app: &mut AppState) {
    // A local bounds failure may close the session before queued native results reach the app.
    let mut ids: Vec<String> = app.claude.prompts.entries.keys().cloned().collect();
    if let Some(active) = &app.active_turn_id {
        if !ids.contains(active) {
            ids.push(active.clone());
        }
    }
    for id in ids {
        if app.record_turn_completed(&id, TurnStatus::Failed) {
            app.turn_start_message_idx = None;
            app.set_status("turn failed");
        }
    }
    app.claude.prompts.entries.clear();
}

pub(super) fn show_error(app: &mut AppState, error: &Value) {
    let message = error["message"].as_str().unwrap_or("Claude SDK error");
    app.append_message(super::Role::System, message.to_string());
    app.set_status(message);
}

fn accept_prompt(app: &mut AppState, params: &Map<String, Value>) -> Option<ServerRequestAction> {
    if params["uuid"].is_string() {
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

fn can_apply(app: &mut AppState, method: &str, params: &Map<String, Value>) -> bool {
    let generation = params.get("generation").and_then(Value::as_u64);
    if generation.is_some_and(|generation| generation != app.claude.generation) {
        return false;
    }
    let process = matches!(
        method,
        "claude/callback" | "claude/callbackCancelled" | "claude/task" | "claude/browserComplete"
    ) || (method == "claude/controlReply"
        && matches!(
            params.get("requestMethod").and_then(Value::as_str),
            Some("start" | "capabilities" | "task.stop" | "usage.optional" | "context")
        ));
    if params
        .get("epoch")
        .and_then(Value::as_u64)
        .is_some_and(|epoch| epoch < app.claude.epoch)
        && !process
    {
        return false;
    }
    if app.claude.phase == super::claude_state::ClaudePhase::Closed
        && !matches!(
            method,
            "claude/error"
                | "claude/closed"
                | "claude/stderr"
                | "claude/callbackCancelled"
                | "claude/promptSubmitted"
                | "claude/promptRejected"
                | "claude/sessionStartFailed"
                | "claude/forkFailed"
        )
    {
        return false;
    }
    if method == "claude/conversationReset" {
        app.claude.epoch = params
            .get("epoch")
            .and_then(Value::as_u64)
            .unwrap_or(app.claude.epoch);
    }
    true
}

fn startup_failure(app: &mut AppState, method: &str, params: &Map<String, Value>) {
    let start = method == "claude/sessionStartFailed";
    if start {
        fail_session(app);
    }
    let message = params
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(if start {
            "Claude could not start the edited branch"
        } else {
            "Could not fork Claude session"
        });
    super::claude_edit::failed(app, message);
}

fn receive_history_part(
    app: &mut AppState,
    params: &Map<String, Value>,
) -> Option<ServerRequestAction> {
    match app
        .claude
        .history
        .transfer
        .receive(&Value::Object(params.clone()))
    {
        Ok(Some(value)) => {
            let _ = super::notifications::handle_server_message_line(app, &value.to_string());
        }
        Ok(None) => {}
        Err(error) => {
            show_error(app, &serde_json::json!({"message":error.to_string()}));
            fail_session(app);
            return None;
        }
    }
    Some(ServerRequestAction::ClaudeControl {
        method: "history.continue".to_string(),
        params: serde_json::json!({"generation":params["generation"],"epoch":params["epoch"]}),
    })
}
