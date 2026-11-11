//! Reads native control replies without assigning turn completion to acknowledgements.

use serde_json::Value;

use super::claude_models::apply_capabilities;
use super::claude_notifications::{fail_session, show_error};
use super::notifications::ServerRequestAction;
use super::state::AppState;

/// Apply a correlated control answer, with bounded task-run and context freshness checks.
pub(super) fn receive(
    app: &mut AppState,
    frame: &Value,
    request: Option<&str>,
    context: &Value,
) -> Option<ServerRequestAction> {
    if frame["ok"] != true {
        return failed(app, frame, request, context);
    }
    match request {
        Some(method @ ("sessions.info" | "sessions.read")) => {
            match super::claude_session_history::receive(app, method, &frame["result"]) {
                Ok(action) => return Some(action),
                Err(error) => {
                    show_error(app, &serde_json::json!({"message":error.to_string()}));
                    fail_session(app);
                }
            }
        }
        Some("start" | "capabilities") => {
            apply_capabilities(app, &frame["result"]);
            return Some(ServerRequestAction::ApplyClaudeSettings);
        }
        Some("task.stop") => {
            if let Some(id) = context["taskId"].as_str() {
                app.claude
                    .tasks
                    .stop_reply(id, context["runId"].as_str(), None);
            }
        }
        Some("usage.optional") => {
            app.claude
                .quota
                .optional(&frame["result"], context["revision"].as_u64().unwrap_or(0));
        }
        Some("context") => {
            super::claude_context::receive(app, frame, context);
        }
        _ => {}
    }
    None
}

fn failed(
    app: &mut AppState,
    frame: &Value,
    request: Option<&str>,
    context: &Value,
) -> Option<ServerRequestAction> {
    if request == Some("task.stop") {
        if let Some(id) = context["taskId"].as_str() {
            app.claude.tasks.stop_reply(
                id,
                context["runId"].as_str(),
                Some(
                    frame["error"]["message"]
                        .as_str()
                        .unwrap_or("Stop failed")
                        .to_string(),
                ),
            );
        }
        return None;
    }
    if request == Some("context") {
        super::claude_context::receive(app, frame, context);
        return None;
    }
    if request == Some("usage.optional") {
        return None;
    }
    show_error(app, &frame["error"]);
    if matches!(request, Some("start" | "sessions.info" | "sessions.read")) {
        fail_session(app);
    }
    None
}
