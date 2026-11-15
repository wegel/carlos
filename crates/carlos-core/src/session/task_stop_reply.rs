//! Translate native task stop acknowledgements into private outcomes without completing tasks.

use carlos_proto::ids::{TaskId, TaskRun};
use carlos_proto::submissions::{ClientOperation, OperationState};
use carlos_proto::tasks::TaskKind;
use serde_json::Value;

/// Only the originating operation receives this result; native task events carry actual state.
pub struct TaskStopReply {
    pub operation: ClientOperation,
    pub task: TaskId,
    pub run: TaskRun,
    pub kind: TaskKind,
    pub state: OperationState,
}

pub(in crate::session) fn codex(params: &Value) -> Option<TaskStopReply> {
    let context = &params["requestContext"];
    if context["operation"] != "taskStop" {
        return None;
    }
    let kind = match params["requestMethod"].as_str()? {
        "turn/interrupt" => TaskKind::Agent,
        "thread/backgroundTerminals/terminate" => TaskKind::Process,
        _ => return None,
    };
    let response = &params["response"];
    let state = if let Some(error) = response.get("error") {
        native_error(
            error,
            matches!(error["code"].as_i64(), Some(-32601 | -32602)),
        )
    } else if response["result"].is_object()
        && (kind != TaskKind::Process || response["result"]["terminated"].is_boolean())
    {
        OperationState::Accepted {
            entry: None,
            notice: None,
        }
    } else {
        uncertain("Native task stop acknowledgement is malformed; stop was not confirmed")
    };
    from_context(context, kind, state)
}

pub(in crate::session) fn claude(frame: &Value, context: &Value) -> Option<TaskStopReply> {
    let owned = context.get("taskStopContext")?;
    let state = if frame["ok"] != true {
        let definite = matches!(
            frame["error"]["code"].as_str(),
            Some(
                "stale_task_run"
                    | "task_run_unavailable"
                    | "invalid_request"
                    | "not_ready"
                    | "unknown_method"
            )
        );
        native_error(&frame["error"], definite)
    } else if context["taskId"]
        .as_str()
        .is_some_and(|id| !id.is_empty() && id.len() <= 4096)
        && frame["result"]["taskId"] == context["taskId"]
        && frame["result"]["stopRequested"] == true
    {
        OperationState::Accepted {
            entry: None,
            notice: None,
        }
    } else {
        uncertain("Native task stop acknowledgement does not match its original task")
    };
    from_context(owned, TaskKind::Task, state)
}

fn from_context(context: &Value, kind: TaskKind, state: OperationState) -> Option<TaskStopReply> {
    Some(TaskStopReply {
        operation: serde_json::from_value(context["clientOperation"].clone()).ok()?,
        task: serde_json::from_value(context["task"].clone()).ok()?,
        run: serde_json::from_value(context["expectedRun"].clone()).ok()?,
        kind,
        state,
    })
}

fn native_error(error: &Value, definite: bool) -> OperationState {
    let message: String = error["message"]
        .as_str()
        .unwrap_or("Native task stop was not confirmed")
        .chars()
        .take(512)
        .collect();
    let message = super::text::safe_text(&message);
    if definite {
        OperationState::Rejected {
            message,
            recovery: None,
        }
    } else {
        uncertain(&message)
    }
}

fn uncertain(message: &str) -> OperationState {
    OperationState::Uncertain {
        message: message.into(),
        recovery: None,
    }
}
