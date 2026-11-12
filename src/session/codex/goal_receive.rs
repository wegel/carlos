//! Apply only current-root native goal facts, preserving newer events and independent drafts.

use serde_json::Value;

use super::goal_state::{GoalKnowledge, GoalRequest};
use super::goals::GoalUpdate;
use super::scope::CodexScope;
use crate::session::state::SessionState;

/// Keep native root goal notifications independent of child turns and UI editor focus.
pub(crate) fn observed(
    session: &mut SessionState,
    update: &mut GoalUpdate,
    method: &str,
    params: &Value,
) {
    if params["threadId"] != *session.thread_id() {
        return;
    }
    if method != "thread/goal/cleared" && !params["goal"].is_object() {
        invalid(
            session,
            update,
            "Codex goal update has no valid goal object; r refreshes native state",
        );
        return;
    }
    let goal = if method == "thread/goal/cleared" {
        Value::Null
    } else {
        params["goal"].clone()
    };
    if let Err(error) = apply(session, update, &goal) {
        invalid(session, update, &error);
    }
}

/// Match exact request params and epoch before accepting reads or mutation results.
pub(crate) fn reply(
    session: &mut SessionState,
    scope: CodexScope,
    update: &mut GoalUpdate,
    params: &Value,
) -> bool {
    if params["requestContext"]["operation"] != "goal" {
        return false;
    }
    if params["requestContext"]["epoch"].as_u64() != Some(scope.epoch)
        || params["requestParams"]["threadId"] != *session.thread_id()
    {
        return true;
    }
    let matches = |request: &GoalRequest| {
        params["requestContext"]["token"].as_u64() == Some(request.token)
            && params["requestMethod"] == request.method
            && params["requestParams"] == request.params
    };
    if session.codex_goals.read.as_ref().is_some_and(matches) {
        let request = session.codex_goals.read.take().expect("goal read");
        if request.revision == session.codex_goals.revision {
            read_reply(session, update, &params["response"]);
        }
    } else if session.codex_goals.mutation.as_ref().is_some_and(matches) {
        let request = session.codex_goals.mutation.take().expect("goal mutation");
        mutation_reply(session, update, &request, &params["response"]);
    }
    true
}

fn read_reply(session: &mut SessionState, update: &mut GoalUpdate, response: &Value) {
    if let Some(error) = response.get("error") {
        let message = error["message"]
            .as_str()
            .unwrap_or("Codex goal read failed");
        session.codex_goals.knowledge = if crate::protocol_history::is_unsupported_method(error)
            || message == "goals feature is disabled"
        {
            GoalKnowledge::Unavailable
        } else {
            GoalKnowledge::Unknown
        };
        session.codex_goals.read_error = Some(crate::session::text::safe_text(message));
        super::goals::reconcile_ralph(session, update);
        return;
    }
    let result = &response["result"];
    let outcome = result
        .get("goal")
        .ok_or_else(|| "Codex did not return goal state; r refreshes".to_string())
        .and_then(|goal| apply(session, update, goal));
    if let Err(error) = outcome {
        session.codex_goals.knowledge = GoalKnowledge::Unknown;
        session.codex_goals.read_error = Some(error);
        super::goals::reconcile_ralph(session, update);
    }
}

fn mutation_reply(
    session: &mut SessionState,
    update: &mut GoalUpdate,
    request: &GoalRequest,
    response: &Value,
) {
    session.codex_goals.notice.clear();
    if let Some(error) = response.get("error") {
        if request.may_activate && !matches!(error["code"].as_i64(), Some(-32602..=-32600)) {
            invalid(
                session,
                update,
                "Goal activation is unconfirmed; Ralph waits for a fresh snapshot",
            );
        }
        update.clear_error = true;
        session.codex_goals.error = Some(format!(
            "Goal change not confirmed: {}. Draft kept; no automatic retry",
            crate::session::text::safe_text(
                error["message"].as_str().unwrap_or("native request failed")
            )
        ));
        session.codex_goals.last_read = None;
        return;
    }
    finish_mutation(session, update, request, response);
}

fn finish_mutation(
    session: &mut SessionState,
    update: &mut GoalUpdate,
    request: &GoalRequest,
    response: &Value,
) {
    let result = &response["result"];
    let outcome = if request.method == "thread/goal/clear" {
        if result["cleared"].is_boolean() {
            invalid(
                session,
                update,
                "Goal clear acknowledged; waiting for a fresh native snapshot",
            );
            Ok(())
        } else {
            Err("Codex did not confirm whether the goal was cleared".into())
        }
    } else if request.revision == session.codex_goals.revision {
        result
            .get("goal")
            .filter(|goal| goal.is_object())
            .ok_or_else(|| "Codex did not return the changed goal".to_string())
            .and_then(|goal| apply(session, update, goal))
    } else {
        if request.may_activate {
            invalid(
                session,
                update,
                "Goal activation awaits a fresh native snapshot; Ralph stays off",
            );
        }
        Ok(())
    };
    if let Err(error) = outcome {
        invalid(session, update, &error);
        update.clear_error = true;
        session.codex_goals.error = Some(error);
    } else {
        session.codex_goals.notice =
            "Codex acknowledged the goal change; execution/status remain native-owned".into();
        if request.params.get("objective").is_some() {
            update.clear_editor = true;
        }
    }
    session.codex_goals.last_read = None;
}

fn apply(session: &mut SessionState, update: &mut GoalUpdate, goal: &Value) -> Result<(), String> {
    validate(&session.thread_id(), goal)?;
    session.codex_goals.read_error = None;
    session.codex_goals.goal = goal.clone();
    session.codex_goals.knowledge = GoalKnowledge::Known;
    session.codex_goals.revision = session.codex_goals.revision.saturating_add(1);
    super::goals::reconcile_ralph(session, update);
    Ok(())
}

fn invalid(session: &mut SessionState, update: &mut GoalUpdate, error: &str) {
    session.codex_goals.knowledge = GoalKnowledge::Unknown;
    session.codex_goals.revision = session.codex_goals.revision.saturating_add(1);
    session.codex_goals.last_read = None;
    session.codex_goals.read_error = Some(error.into());
    super::goals::reconcile_ralph(session, update);
}

/// Reject malformed root snapshots before the UI replaces any history.
pub(crate) fn validate(thread: &str, goal: &Value) -> Result<(), String> {
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
