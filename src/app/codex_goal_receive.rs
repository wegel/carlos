//! Apply only current-root native goal facts, preserving newer events and independent drafts.

use serde_json::Value;

use super::codex_goal_state::{GoalKnowledge, GoalRequest};
use super::state::AppState;

/// Keep native root goal notifications independent of child turns and UI editor focus.
pub(super) fn observed(app: &mut AppState, method: &str, params: &Value) {
    if params["threadId"] != app.thread_id {
        return;
    }
    if method != "thread/goal/cleared" && !params["goal"].is_object() {
        invalid(
            app,
            "Codex goal update has no valid goal object; r refreshes native state",
        );
        return;
    }
    let goal = if method == "thread/goal/cleared" {
        Value::Null
    } else {
        params["goal"].clone()
    };
    if let Err(error) = apply(app, &goal) {
        invalid(app, &error);
    }
}

/// Match exact request params and epoch before accepting reads or mutation results.
pub(super) fn reply(app: &mut AppState, params: &Value) -> bool {
    if params["requestContext"]["operation"] != "goal" {
        return false;
    }
    if params["requestContext"]["epoch"].as_u64() != Some(app.codex.epoch)
        || params["requestParams"]["threadId"] != app.thread_id
    {
        return true;
    }
    let matches = |request: &GoalRequest| {
        params["requestContext"]["token"].as_u64() == Some(request.token)
            && params["requestMethod"] == request.method
            && params["requestParams"] == request.params
    };
    if app.codex.goals.read.as_ref().is_some_and(matches) {
        let request = app.codex.goals.read.take().expect("goal read");
        if request.revision == app.codex.goals.revision {
            read_reply(app, &params["response"]);
        }
    } else if app.codex.goals.mutation.as_ref().is_some_and(matches) {
        let request = app.codex.goals.mutation.take().expect("goal mutation");
        mutation_reply(app, &request, &params["response"]);
    }
    true
}

fn read_reply(app: &mut AppState, response: &Value) {
    if let Some(error) = response.get("error") {
        let message = error["message"]
            .as_str()
            .unwrap_or("Codex goal read failed");
        app.codex.goals.knowledge = if crate::protocol_history::is_unsupported_method(error)
            || message == "goals feature is disabled"
        {
            GoalKnowledge::Unavailable
        } else {
            GoalKnowledge::Unknown
        };
        app.codex.goals.read_error = Some(super::claude_question_state::safe_text(message));
        super::codex_goals::reconcile_ralph(app);
        return;
    }
    let result = &response["result"];
    let outcome = result
        .get("goal")
        .ok_or_else(|| "Codex did not return goal state; r refreshes".to_string())
        .and_then(|goal| apply(app, goal));
    if let Err(error) = outcome {
        app.codex.goals.knowledge = GoalKnowledge::Unknown;
        app.codex.goals.read_error = Some(error);
        super::codex_goals::reconcile_ralph(app);
    }
}

fn mutation_reply(app: &mut AppState, request: &GoalRequest, response: &Value) {
    app.codex.goals.notice.clear();
    if let Some(error) = response.get("error") {
        if request.may_activate && !matches!(error["code"].as_i64(), Some(-32602..=-32600)) {
            invalid(
                app,
                "Goal activation is unconfirmed; Ralph waits for a fresh snapshot",
            );
        }
        app.codex.goals.error = Some(format!(
            "Goal change not confirmed: {}. Draft kept; no automatic retry",
            super::claude_question_state::safe_text(
                error["message"].as_str().unwrap_or("native request failed")
            )
        ));
        app.codex.goals.last_read = None;
        return;
    }
    let result = &response["result"];
    let outcome = if request.method == "thread/goal/clear" {
        if result["cleared"].is_boolean() {
            invalid(
                app,
                "Goal clear acknowledged; waiting for a fresh native snapshot",
            );
            Ok(())
        } else {
            Err("Codex did not confirm whether the goal was cleared".into())
        }
    } else if request.revision == app.codex.goals.revision {
        result
            .get("goal")
            .filter(|goal| goal.is_object())
            .ok_or_else(|| "Codex did not return the changed goal".to_string())
            .and_then(|goal| apply(app, goal))
    } else {
        if request.may_activate {
            invalid(
                app,
                "Goal activation awaits a fresh native snapshot; Ralph stays off",
            );
        }
        Ok(())
    };
    if let Err(error) = outcome {
        invalid(app, &error);
        app.codex.goals.error = Some(error);
    } else {
        app.codex.goals.notice =
            "Codex acknowledged the goal change; execution/status remain native-owned".into();
        if request.params.get("objective").is_some() {
            app.codex.goals.editor = None;
            app.codex.goals.editing = false;
        }
    }
    app.codex.goals.last_read = None;
}

fn apply(app: &mut AppState, goal: &Value) -> Result<(), String> {
    validate(&app.thread_id, goal)?;
    app.codex.goals.read_error = None;
    app.codex.goals.goal = goal.clone();
    app.codex.goals.knowledge = GoalKnowledge::Known;
    app.codex.goals.revision = app.codex.goals.revision.saturating_add(1);
    super::codex_goals::reconcile_ralph(app);
    Ok(())
}

fn invalid(app: &mut AppState, error: &str) {
    app.codex.goals.knowledge = GoalKnowledge::Unknown;
    app.codex.goals.revision = app.codex.goals.revision.saturating_add(1);
    app.codex.goals.last_read = None;
    app.codex.goals.read_error = Some(error.into());
    super::codex_goals::reconcile_ralph(app);
}

/// Reject malformed root snapshots before the UI replaces any history.
pub(super) fn validate(thread: &str, goal: &Value) -> Result<(), String> {
    if goal.to_string().len() > 128 * 1024 {
        return Err(
            "Codex goal exceeds128 KiB; controls unavailable until a readable snapshot arrives"
                .into(),
        );
    }
    if !goal.is_null()
        && (goal["threadId"] != thread
            || !goal["objective"].is_string()
            || !goal["status"].is_string()
            || ["tokensUsed", "timeUsedSeconds"]
                .iter()
                .any(|key| goal[key].as_i64().is_none_or(|value| value < 0))
            || ["createdAt", "updatedAt"]
                .iter()
                .any(|key| goal[key].as_i64().is_none())
            || goal.get("tokenBudget").is_some_and(|value| {
                !value.is_null() && value.as_i64().is_none_or(|budget| budget <= 0)
            }))
    {
        return Err("Codex goal returned malformed or foreign thread data".into());
    }
    Ok(())
}
