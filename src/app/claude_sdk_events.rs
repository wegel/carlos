//! Applies named Claude facts after the backend has translated native transcript messages.

use serde_json::Value;

use super::models::Role;
use super::state::AppState;
use crate::session::actions::ServerRequestAction;

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
        "claude/activity" => app.session.record_claude_activity(facts),
        "claude/status" => return status(app, facts),
        "claude/thinking" => app.session.record_claude_thinking(facts),
        "claude/result" => {
            if let Some(error) = facts["error"].as_str() {
                app.append_message(Role::System, error.to_string());
            }
            app.session.finish_claude_activity();
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
                    app.session.remove_claude_prompt(id);
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
        app.session.record_claude_status(facts);
    }
    (facts["refreshContext"] == true).then_some(ServerRequestAction::RefreshContext)
}

fn task(app: &mut AppState, message: &Value) -> Option<ServerRequestAction> {
    if let Err(error) = app.session.update_claude_task(message) {
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
