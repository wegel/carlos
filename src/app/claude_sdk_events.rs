//! Applies named Claude facts after the backend has translated native transcript messages.

use serde_json::Value;

use super::models::Role;
use super::notifications::ServerRequestAction;
use super::state::AppState;

/// Apply plain backend facts to the existing Claude controls and notices.
pub(super) fn receive(
    app: &mut AppState,
    method: &str,
    facts: &Value,
) -> Option<ServerRequestAction> {
    match method {
        "claude/quota" => app.session.claude_quota_event(facts),
        "claude/browserComplete" => super::claude_browser::complete(app, facts),
        "claude/task" => return task(app, facts),
        "claude/conversationReset" => super::claude_reset::receive(app, facts),
        "claude/activity" => {
            app.claude.activity.state(facts);
            if facts["state"] == "running" {
                super::claude_context::refresh(app);
            }
        }
        "claude/status" => return status(app, facts),
        "claude/thinking" => app.claude.activity.thinking = facts["estimated_tokens"].as_u64(),
        "claude/result" => {
            if let Some(error) = facts["error"].as_str() {
                app.append_message(Role::System, error.to_string());
            }
            app.claude.activity.transient = None;
            app.claude.activity.thinking = None;
            return Some(ServerRequestAction::RefreshContext);
        }
        "claude/usageReport" => super::claude_notices::usage(app, facts),
        "claude/notice" => {
            super::claude_notices::receive(app, facts);
        }
        "claude/initialized" => {
            super::claude_reset::initialize(app, facts);
            if let Some(model) = facts["model"].as_str() {
                super::claude_models::observe_model(app, model);
            }
        }
        "claude/promptsRetired" => {
            if let Some(ids) = facts["ids"].as_array() {
                for id in ids.iter().filter_map(Value::as_str) {
                    app.claude.prompts.entries.remove(id);
                }
            }
        }
        "claude/promptsConsumed" => {
            let ids = consumed_ids(facts);
            super::claude_prompts::consume(app, &ids);
        }
        _ => {}
    }
    None
}

fn status(app: &mut AppState, facts: &Value) -> Option<ServerRequestAction> {
    if facts["compaction"] == true {
        super::claude_notices::compact(app, facts);
    } else {
        app.claude.activity.transient = facts["label"].as_str().map(str::to_string);
    }
    (facts["refreshContext"] == true).then_some(ServerRequestAction::RefreshContext)
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

fn consumed_ids(facts: &Value) -> Vec<String> {
    facts["ids"]
        .as_array()
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}
