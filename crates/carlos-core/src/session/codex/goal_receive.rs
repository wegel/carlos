//! Apply only current-root native goal facts, preserving newer events and independent drafts.

use carlos_proto::submissions::OperationState;
use serde_json::Value;

use super::goal_state::{GoalKnowledge, GoalRequest};
use super::goals::GoalUpdate;
use super::scope::CodexScope;
use crate::session::state::SessionState;

/// Keep native root goal notifications independent of child turns and UI editor focus.
pub fn observed(session: &mut SessionState, method: &str, params: &Value) {
    if params["threadId"] != *session.thread_id() {
        return;
    }
    if method != "thread/goal/cleared" && !params["goal"].is_object() {
        invalid(
            session,
            "Codex goal update has no valid goal object; refresh native state",
        );
        return;
    }
    let goal = if method == "thread/goal/cleared" {
        Value::Null
    } else {
        params["goal"].clone()
    };
    if let Err(error) = apply(session, &goal) {
        invalid(session, &error);
    }
}

/// Match exact request params and epoch before accepting reads or mutation results.
pub fn reply(
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
            read_reply(session, &params["response"]);
        }
    } else if session.codex_goals.mutation.as_ref().is_some_and(matches) {
        let request = session.codex_goals.mutation.take().expect("goal mutation");
        mutation_reply(session, update, &request, &params["response"]);
    }
    true
}

fn read_reply(session: &mut SessionState, response: &Value) {
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
        session.codex_goals.read_error = Some(crate::protocol_events::bounded_diagnostic(message));
        super::goals::reconcile_ralph(session);
        return;
    }
    let result = &response["result"];
    let outcome = result
        .get("goal")
        .ok_or_else(|| "Codex did not return goal state; refresh it".to_string())
        .and_then(|goal| apply_read(session, goal));
    if let Err(error) = outcome {
        session.codex_goals.knowledge = GoalKnowledge::Unknown;
        session.codex_goals.read_error = Some(error);
        super::goals::reconcile_ralph(session);
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
                "Goal activation is unconfirmed; Ralph waits for a fresh snapshot",
            );
        }
        update.clear_error = true;
        session.codex_goals.error = Some(format!(
            "Goal change not confirmed: {}. Draft kept; no automatic retry",
            crate::protocol_events::bounded_diagnostic(
                error["message"].as_str().unwrap_or("native request failed")
            )
        ));
        session.codex_goals.last_read = None;
        if let Some(operation) = &request.operation {
            let message = session.codex_goals.error.clone().expect("mutation failure");
            let state = if matches!(error["code"].as_i64(), Some(-32602..=-32600)) {
                OperationState::Rejected {
                    message,
                    recovery: None,
                }
            } else {
                OperationState::Uncertain {
                    message,
                    recovery: None,
                }
            };
            update.outcome = Some((operation.clone(), state));
        }
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
            .and_then(|goal| apply(session, goal))
    } else {
        if request.may_activate {
            invalid(
                session,
                "Goal activation awaits a fresh native snapshot; Ralph stays off",
            );
        }
        Ok(())
    };
    if let Err(error) = outcome {
        invalid(session, &error);
        update.clear_error = true;
        session.codex_goals.error = Some(error);
        set_outcome(
            update,
            request,
            OperationState::Uncertain {
                message: session
                    .codex_goals
                    .error
                    .clone()
                    .expect("invalid native acknowledgement"),
                recovery: None,
            },
        );
    } else {
        session.codex_goals.notice =
            "Codex acknowledged the goal change; execution/status remain native-owned".into();
        if request.params.get("objective").is_some() {
            update.clear_editor = true;
        }
        set_outcome(update, request, OperationState::Accepted { entry: None });
    }
    session.codex_goals.last_read = None;
}

fn set_outcome(update: &mut GoalUpdate, request: &GoalRequest, state: OperationState) {
    if let Some(operation) = &request.operation {
        update.outcome = Some((operation.clone(), state));
    }
}

fn apply(session: &mut SessionState, goal: &Value) -> Result<(), String> {
    validate(session.thread_id(), goal)?;
    let revision = session.codex_goals.revision.checked_add(1).ok_or_else(|| {
        "native goal revision counter exhausted; reconnect before changing goals".to_owned()
    })?;
    session.codex_goals.goal = goal.clone();
    session.codex_goals.revision = revision;
    session.codex_goals.read_error = None;
    session.codex_goals.knowledge = GoalKnowledge::Known;
    super::goals::reconcile_ralph(session);
    Ok(())
}

fn apply_read(session: &mut SessionState, goal: &Value) -> Result<(), String> {
    if session.codex_goals.knowledge == GoalKnowledge::Known && &session.codex_goals.goal == goal {
        // Equal reads do not expire an editor. Notifications still advance the
        // revision even when equal: their arrival fences a racing activation ACK.
        session.codex_goals.read_error = None;
        Ok(())
    } else {
        apply(session, goal)
    }
}

fn invalid(session: &mut SessionState, error: &str) {
    session.codex_goals.knowledge = GoalKnowledge::Unknown;
    session.codex_goals.revision = session.codex_goals.revision.saturating_add(1);
    session.codex_goals.last_read = None;
    session.codex_goals.read_error = Some(crate::protocol_events::bounded_diagnostic(error));
    super::goals::reconcile_ralph(session);
}

/// Reject malformed root snapshots before the UI replaces any history.
pub fn validate(thread: &str, goal: &Value) -> Result<(), String> {
    if goal.to_string().len() > carlos_proto::goals::MAX_GOAL_BYTES {
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
