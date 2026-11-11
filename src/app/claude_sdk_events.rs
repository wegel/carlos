//! Reads native events before translating transcript items at the shared UI boundary.

use serde_json::Value;

use crate::claude_backend::{is_child_message, is_child_report_notice, translate_claude_line};

use super::claude_models::observe_model;
use super::claude_state::prompt_ids;
use super::models::Role;
use super::notifications::{handle_server_message_line, ServerRequestAction};
use super::state::AppState;

/// Process raw SDK state, child events, and main conversation items without synthetic turns.
pub(super) fn receive(app: &mut AppState, message: &Value) -> Option<ServerRequestAction> {
    if message["type"] == "rate_limit_event" {
        app.claude.quota.event(&message["rate_limit_info"]);
        return None;
    }
    if message["type"] == "system" && message["subtype"] == "elicitation_complete" {
        super::claude_browser::complete(app, message);
        return None;
    }
    if task_message(message) {
        return task(app, message);
    }
    if is_child_message(message) || is_child_report_notice(message) {
        return None;
    }
    if message["type"] == "conversation_reset" {
        super::claude_reset::receive(app, message);
        return None;
    }
    match (message["type"].as_str(), message["subtype"].as_str()) {
        (Some("system"), Some("session_state_changed")) => {
            app.claude.activity.state(message);
            if message["state"] == "running" {
                super::claude_context::refresh(app);
            }
            app.claude.set_idle(message["state"] == "idle");
            finish_results(app);
            None
        }
        (Some("system"), Some("status" | "compact_boundary")) => status(app, message),
        (Some("system"), Some("thinking_tokens")) => {
            app.claude.activity.thinking = message["estimated_tokens"].as_u64();
            None
        }
        (Some("result"), _) => {
            app.claude.result(message);
            if message["is_error"] == true || message["subtype"] != "success" {
                app.append_message(
                    Role::System,
                    super::claude_notifications::result_error(message),
                );
            }
            app.claude.activity.transient = None;
            app.claude.activity.thinking = None;
            finish_results(app);
            Some(ServerRequestAction::RefreshContext)
        }
        _ => {
            main_items(app, message);
            None
        }
    }
}

/// Complete owned prompt UUIDs only after native result and idle have both arrived.
pub(super) fn finish_results(app: &mut AppState) {
    for line in app.claude.completions() {
        let _ = handle_server_message_line(app, &line);
    }
}

fn main_items(app: &mut AppState, message: &Value) {
    if message["type"] == "assistant" && !message["usage_report"].is_null() {
        super::claude_notices::usage(app, message);
    }
    if message["type"] == "system" && super::claude_notices::receive(app, message) {
        return;
    }
    if message["type"] == "system" && message["subtype"] == "init" {
        super::claude_reset::initialize(app, message);
        if let Some(model) = message["model"].as_str() {
            observe_model(app, model);
        }
    }
    if matches!(message["type"].as_str(), Some("stream_event" | "assistant"))
        || message["subtype"] == "init"
    {
        app.claude.bind_reply(message);
        super::claude_prompts::consume(app, &prompt_ids(message));
    }
    if message["event"]["type"] == "content_block_start"
        && !matches!(
            message["event"]["content_block"]["type"].as_str(),
            Some("thinking" | "redacted_thinking")
        )
    {
        app.claude.activity.thinking = None;
    }
    if let Ok(output) = translate_claude_line(&mut app.claude.translation, &message.to_string()) {
        for line in output.lines {
            let notification = serde_json::from_str::<Value>(&line).unwrap_or(Value::Null);
            if !matches!(
                notification["method"].as_str(),
                Some(
                    "thread/tokenUsage/updated"
                        | "claude/exitPlan/requestApproval"
                        | "turn/started"
                        | "turn/completed"
                )
            ) {
                let _ = handle_server_message_line(app, &line);
            }
        }
    }
}

fn status(app: &mut AppState, message: &Value) -> Option<ServerRequestAction> {
    if message["status"] == "compacting"
        || !message["compact_result"].is_null()
        || message["subtype"] == "compact_boundary"
    {
        super::claude_notices::compact(app, message);
        if message["status"] != "compacting" {
            return Some(ServerRequestAction::RefreshContext);
        }
    } else {
        app.claude.activity.transient = message["status"].as_str().map(|status| match status {
            "requesting" => "Requesting".to_string(),
            other => other.to_string(),
        });
    }
    None
}

fn task_message(message: &Value) -> bool {
    message["type"] == "system"
        && matches!(
            message["subtype"].as_str(),
            Some(
                "task_started"
                    | "task_updated"
                    | "task_progress"
                    | "task_notification"
                    | "background_tasks_changed"
            )
        )
}

fn task(app: &mut AppState, message: &Value) -> Option<ServerRequestAction> {
    if let Err(error) = app.claude.tasks.update(message) {
        return bounded_failure(app, error);
    }
    None
}

fn bounded_failure(app: &mut AppState, error: &str) -> Option<ServerRequestAction> {
    app.append_message(Role::System, error.to_string());
    super::claude_notifications::fail_session(app);
    Some(ServerRequestAction::ClaudeControl {
        method: "shutdown".to_string(),
        params: serde_json::json!({}),
    })
}
