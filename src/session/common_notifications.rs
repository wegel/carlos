//! Apply the existing common JSON message subset through the session owner.

use serde_json::Value;

use super::actions::ServerRequestAction;
use super::codex::approval_parsing::pending_approval_from_request;
use super::common_update::{ApprovalChanged, CommonUpdate};
use super::context_parse::{
    context_usage_from_thread_token_usage_params, context_usage_from_token_count_params,
};
use super::notification_items::handle_item_notification;
use super::notification_turns::{handle_turn_completed, handle_turn_started};
use super::state::SessionState;
use super::tools::command_summary_from_parsed_cmd;

impl SessionState {
    /// Process shared JSON without constructing a client or invoking backend extension handlers.
    #[cfg(test)]
    pub(crate) fn handle_common_backend_line(&mut self, line: &str) -> CommonUpdate {
        let Ok(parsed) = serde_json::from_str::<Value>(line) else {
            return CommonUpdate::default();
        };
        if self.codex_scope().closed() || self.rate_limits_response(&parsed) {
            return CommonUpdate::default();
        }
        let previous =
            self.replace_codex_item_turn(parsed["params"]["turnId"].as_str().map(str::to_string));
        let update = self.handle_common_backend_message(&parsed);
        self.replace_codex_item_turn(previous);
        update
    }

    /// Apply a parsed common message after the backend-specific routers have accepted it.
    pub(crate) fn handle_common_backend_message(&mut self, parsed: &Value) -> CommonUpdate {
        let mut update = CommonUpdate::default();
        let Some(method) = parsed.get("method").and_then(Value::as_str) else {
            return update;
        };
        let Some(params) = parsed.get("params").and_then(Value::as_object) else {
            return update;
        };
        if params
            .get("threadId")
            .and_then(Value::as_str)
            .is_some_and(|id| id != *self.thread_id())
        {
            return update;
        }
        if let Some(request_id) = parsed.get("id").cloned() {
            if let Some(prompt) = pending_approval_from_request(method, params, request_id.clone())
            {
                let before = self.approval().pending().map(|pending| pending.entry_id());
                let retired = self.set_pending_approval(prompt);
                let current = self.approval().pending().map(|pending| pending.entry_id());
                update.approval = Some(ApprovalChanged {
                    clear_extra_lines: current == before,
                    current,
                    retired,
                });
            } else {
                update.action = Some(ServerRequestAction::ReplyError {
                    request_id,
                    code: -32601,
                    message: format!("unsupported server request: {method}"),
                });
            }
            return update;
        }
        if !handle_item_notification(self, &mut update.items, method, params) {
            dispatch_notification(self, &mut update, method, params);
        }
        update
    }
}

fn dispatch_notification(
    session: &mut SessionState,
    update: &mut CommonUpdate,
    method: &str,
    params: &serde_json::Map<String, Value>,
) {
    if handle_context_notification(session, update, method, params) {
        return;
    }
    match method {
        "account/rateLimits/updated" => {
            if let Some(snapshot) = params.get("rateLimits") {
                session.update_rate_limits(snapshot);
            }
        }
        "account/updated" => {
            session.clear_rate_limits();
            update.clear_quota_countdown = true;
        }
        "thread/initialized" => handle_thread_initialized(session, update, params),
        "thread/name/updated" => {
            if params.get("threadId").and_then(Value::as_str) == Some(session.thread_id().as_str())
            {
                session.observe_name(params.get("threadName").and_then(Value::as_str));
            }
        }
        "turn/started" => handle_turn_started(session, params),
        "turn/completed" => handle_turn_completed(session, update, params),
        "turn/diff/updated" => {
            if let (Some(turn_id), Some(diff)) = (
                params.get("turnId").and_then(Value::as_str),
                params.get("diff").and_then(Value::as_str),
            ) {
                update.items.push(session.upsert_turn_diff(turn_id, diff));
            }
        }
        "codex/event/turn_diff" => handle_codex_turn_diff(session, update, params),
        "codex/event/exec_command_end" => {
            if let Some(msg) = params.get("msg") {
                if let Some((call_id, summary)) = command_summary_from_parsed_cmd(msg) {
                    update.items.push(
                        session.set_command_override(&session.backend_entry_key(&call_id), summary),
                    );
                }
            }
        }
        "error" => {
            let msg = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("server error");
            session.set_status(msg);
        }
        _ => {}
    }
}

fn handle_thread_initialized(
    session: &mut SessionState,
    update: &mut CommonUpdate,
    params: &serde_json::Map<String, Value>,
) {
    if let Some(id) = params
        .get("thread")
        .and_then(Value::as_object)
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if session.thread_id() != id {
            session.set_context_usage(None);
        }
        update.thread = Some(session.adopt_native_thread(id.to_string()));
    }
    if let Some(thread) = params.get("thread") {
        session.observe_thread_name(thread);
    }
    let model = params
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(ToOwned::to_owned);
    let effort = params
        .get("reasoningEffort")
        .or_else(|| params.get("effort"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(ToOwned::to_owned);
    let summary = params
        .get("summary")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    if model.is_some() || effort.is_some() || summary.is_some() {
        session.merge_runtime_settings(model, effort, summary);
    }
}

fn handle_codex_turn_diff(
    session: &mut SessionState,
    update: &mut CommonUpdate,
    params: &serde_json::Map<String, Value>,
) {
    let turn_id = params
        .get("id")
        .and_then(Value::as_str)
        .or_else(|| params.get("turnId").and_then(Value::as_str));
    let diff = params
        .get("msg")
        .and_then(|m| m.get("unified_diff"))
        .and_then(Value::as_str)
        .or_else(|| params.get("diff").and_then(Value::as_str));
    if let (Some(turn_id), Some(diff)) = (turn_id, diff) {
        update.items.push(session.upsert_turn_diff(turn_id, diff));
    }
}

fn handle_context_notification(
    session: &mut SessionState,
    update: &mut CommonUpdate,
    method: &str,
    params: &serde_json::Map<String, Value>,
) -> bool {
    match method {
        "thread/tokenUsage/updated" => {
            if let Some(usage) = context_usage_from_thread_token_usage_params(params) {
                session.set_context_usage(Some(usage));
            }
        }
        "thread/compacted" => {
            session.set_context_usage(None);
            update.items.push(session.append_context_compacted_marker());
        }
        "codex/event/token_count" => {
            if let Some(usage) = context_usage_from_token_count_params(params) {
                session.set_context_usage(Some(usage));
            }
        }
        _ => return false,
    }
    true
}
