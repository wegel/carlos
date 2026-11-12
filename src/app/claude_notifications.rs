//! Applies normalized Claude facts and retains native prompt ownership for the app.

use serde_json::{Map, Value};

use super::claude_callbacks::{cancel_callback, receive_callback};
use super::notifications::ServerRequestAction;
use super::state::AppState;
use crate::session::approval_state::ApprovalQueue;

/// Distinguishes a normalized Claude fact from another backend's shared notification.
pub(super) enum ClaudeRoute {
    Continue,
    Handled(Option<ServerRequestAction>),
}

/// Fence common Claude messages and apply the explicitly named backend facts.
pub(super) fn route(app: &mut AppState, method: &str, params: &Map<String, Value>) -> ClaudeRoute {
    use crate::session::claude::ClaudeRoute as Route;
    match app.session.route_claude_notification(method, params) {
        Route::Continue => ClaudeRoute::Continue,
        Route::Ignore => ClaudeRoute::Handled(None),
        Route::Named => ClaudeRoute::Handled(handle(app, method, params)),
        Route::Context => {
            super::claude_context::receive(
                app,
                &serde_json::json!({"ok":true,"result":params["claudeContext"]}),
                params.get("requestContext").unwrap_or(&Value::Null),
            );
            ClaudeRoute::Handled(None)
        }
        Route::History => {
            app.with_historical_tools(|app| {
                super::notification_items::handle_item_notification(app, method, params);
            });
            ClaudeRoute::Handled(None)
        }
    }
}

/// Attach an accepted native prompt UUID to its visible user message.
pub(super) fn record_submitted_prompt(app: &mut AppState, response: &Value) {
    if let Some(id) = response["result"]["turn"]["id"].as_str() {
        let key = app.item_key(&format!("claude-prompt:{id}"));
        let update = app.session.record_submitted_claude_prompt(response, &key);
        app.apply_session_update(update);
        super::claude_edit::accepted(app, id);
    }
}

/// Handle named Claude facts before the shared app-server message parser.
pub(super) fn handle(
    app: &mut AppState,
    method: &str,
    params: &Map<String, Value>,
) -> Option<ServerRequestAction> {
    app.session.enable_claude();
    if method == "claude/sessionChanged" {
        if let Some((generation, id)) = app.session.claude_session_change(params) {
            super::claude_session_history::switch_session(app, generation, id);
        }
        return None;
    }
    if !app.session.accept_claude_fact(method, params) {
        return None;
    }
    match method {
        "claude/sessionStartFailed" | "claude/forkFailed" => startup_failure(app, method, params),
        "claude/promptSubmitted" => {
            return accept_prompt(app, params);
        }
        "claude/promptRejected" => {
            if let Some(id) = params["uuid"].as_str() {
                app.session.remove_claude_prompt(id);
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
        "claude/historyRecord" => app.session.record_claude_history(params["record"].clone()),
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
    let update = app.session.receive_claude_event(
        method,
        frame,
        super::claude_question_state::safe_label,
        super::claude_question_state::safe_text,
    );
    app.apply_session_update(update.transcript);
    if update.fail_session {
        fail_session(app);
    }
    if update.closed {
        let update = app.session.record_claude_closed(frame);
        app.apply_session_update(update);
    }
    None
}

/// Close the UI query and recover unsent drafts after a terminal helper failure.
pub(super) fn fail_session(app: &mut AppState) {
    super::claude_edit::recover_sending(app);
    super::claude_session_history::restore_edit(app);
    if app.session.close_claude() {
        app.view.claude_edit.failed = true;
    }
    let retired = app.session.clear_approval_queue(ApprovalQueue::Claude);
    app.drop_retired_approval_hints(retired);
    app.clear_pending_approval();
    app.session.fail_claude_session_work();
    super::claude_session_history::recover_queued(app);
}

pub(super) fn show_error(app: &mut AppState, error: &Value) {
    let update = app.session.record_claude_error(error);
    app.apply_session_update(update);
}

fn accept_prompt(app: &mut AppState, params: &Map<String, Value>) -> Option<ServerRequestAction> {
    if params["uuid"].is_string() {
        if let Err(error) = app
            .session
            .register_claude_prompt(&Value::Object(params.clone()))
        {
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
        .session
        .receive_claude_history_part(&Value::Object(params.clone()))
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
