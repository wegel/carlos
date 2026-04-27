//! Server notification routing and animation ticks.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use serde_json::Value;

use super::approval_parsing::pending_approval_from_request;
use super::context_usage::{
    context_usage_from_thread_token_usage_params, context_usage_from_token_count_params,
};
#[cfg(test)]
pub(super) use super::item_history::append_history_from_thread;
use super::notification_items::handle_item_notification;
pub(crate) use super::item_history::load_history_from_start_or_resume;
use super::state::PendingApprovalRequest;
use super::tools::command_summary_from_parsed_cmd;
use super::{AppState, ThreadSummary};
use crate::protocol_params::extract_result_object;
use crate::theme::KITT_STEP_MS;

// --- Approval Routing ---

pub(super) enum ServerRequestAction {
    ReplyError {
        request_id: Value,
        code: i64,
        message: String,
    },
}

// --- Key Helpers ---

pub(crate) fn parse_thread_list(response_line: &str) -> Result<Vec<ThreadSummary>> {
    let parsed = extract_result_object(response_line)?;
    let Some(data) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("data"))
        .and_then(Value::as_array)
    else {
        return Ok(Vec::new());
    };

    let mut out = Vec::new();
    for entry in data {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        let Some(id) = obj.get("id").and_then(Value::as_str) else {
            continue;
        };
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);
        let preview = obj.get("preview").and_then(Value::as_str).unwrap_or("");
        let cwd = obj.get("cwd").and_then(Value::as_str).unwrap_or("");
        let created_at = obj
            .get("createdAt")
            .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
            .unwrap_or(0);
        let updated_at = obj
            .get("updatedAt")
            .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
            .unwrap_or(0);

        out.push(ThreadSummary {
            id: id.to_string(),
            name,
            preview: preview.to_string(),
            cwd: cwd.to_string(),
            created_at,
            updated_at,
        });
    }
    Ok(out)
}

pub(super) fn is_ctrl_char(code: KeyCode, modifiers: KeyModifiers, ch: char) -> bool {
    matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&ch))
        && modifiers.contains(KeyModifiers::CONTROL)
}

pub(super) fn is_perf_toggle_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::F(8)) || is_ctrl_char(code, modifiers, 'p')
}

// --- Animation ---

pub(super) fn animation_tick() -> u128 {
    animation_tick_for_step(KITT_STEP_MS)
}

pub(super) fn animation_tick_for_step(step_ms: u128) -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() / step_ms.max(1))
        .unwrap_or(0)
}

pub(super) fn working_animation_step_ms() -> u128 {
    KITT_STEP_MS
}

pub(super) fn animation_poll_timeout() -> Duration {
    animation_poll_timeout_for_step(working_animation_step_ms())
}

pub(super) fn animation_poll_timeout_for_step(step_ms: u128) -> Duration {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let step_ms = step_ms.max(1);
    let rem = step_ms - (now_ms % step_ms);
    Duration::from_millis(rem.max(1) as u64)
}

pub(super) fn kitt_head_index(width: usize, tick: u128) -> usize {
    if width <= 1 {
        return 0;
    }

    let span = (width - 1) as u128;
    let cycle = span * 2;
    if cycle == 0 {
        return 0;
    }

    let phase = tick % cycle;
    if phase <= span {
        phase as usize
    } else {
        (cycle - phase) as usize
    }
}

pub(super) fn is_key_press_like(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

// --- Server Message Routing ---

pub(crate) fn handle_server_message_line(
    app: &mut AppState,
    line: &str,
) -> Option<ServerRequestAction> {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return None;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return None;
    };
    if let Some(action) = try_handle_server_request(app, method, &parsed) {
        return action;
    }
    let Some(params) = parsed.get("params").and_then(Value::as_object) else {
        return None;
    };
    if handle_item_notification(app, method, params) {
        return None;
    }
    dispatch_notification(app, method, params);
    None
}

fn try_handle_server_request(
    app: &mut AppState,
    method: &str,
    parsed: &Value,
) -> Option<Option<ServerRequestAction>> {
    let request_id = parsed.get("id").cloned()?;
    if let Some(prompt) = parsed
        .get("params")
        .and_then(Value::as_object)
        .and_then(|params| pending_approval_from_request(method, params, request_id.clone()))
    {
        app.set_pending_approval(prompt);
        return Some(None);
    }
    Some(Some(ServerRequestAction::ReplyError {
        request_id,
        code: -32601,
        message: format!("unsupported server request: {method}"),
    }))
}

fn dispatch_notification(
    app: &mut AppState,
    method: &str,
    params: &serde_json::Map<String, Value>,
) {
    match method {
        "thread/initialized" => handle_thread_initialized(app, params),
        "thread/tokenUsage/updated" => {
            if let Some(usage) = context_usage_from_thread_token_usage_params(params) {
                app.context_usage = Some(usage);
            }
        }
        "thread/compacted" => {
            app.context_usage = None;
            app.append_context_compacted_marker();
        }
        "turn/started" => handle_turn_started(app, params),
        "turn/completed" => handle_turn_completed(app, params),
        "turn/diff/updated" => {
            if let (Some(turn_id), Some(diff)) = (
                params.get("turnId").and_then(Value::as_str),
                params.get("diff").and_then(Value::as_str),
            ) {
                app.upsert_turn_diff(turn_id, diff);
            }
        }
        "codex/event/turn_diff" => handle_codex_turn_diff(app, params),
        "codex/event/exec_command_end" => {
            if let Some(msg) = params.get("msg") {
                if let Some((call_id, summary)) = command_summary_from_parsed_cmd(msg) {
                    app.set_command_override(&call_id, summary);
                }
            }
        }
        "codex/event/token_count" => {
            if let Some(usage) = context_usage_from_token_count_params(params) {
                app.context_usage = Some(usage);
            }
        }
        "error" => {
            let msg = params
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("server error");
            app.set_status(msg);
        }
        _ => {}
    }
}

fn handle_thread_initialized(app: &mut AppState, params: &serde_json::Map<String, Value>) {
    if let Some(id) = params
        .get("thread")
        .and_then(Value::as_object)
        .and_then(|thread| thread.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if app.thread_id != id {
            app.context_usage = None;
        }
        app.set_thread_id(id.to_string());
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
        app.merge_runtime_settings(model, effort, summary);
    }
}

fn handle_turn_started(app: &mut AppState, params: &serde_json::Map<String, Value>) {
    app.context_usage = None;
    app.mark_turn_started();
    if let Some(id) = params
        .get("turn")
        .and_then(Value::as_object)
        .and_then(|t| t.get("id"))
        .and_then(Value::as_str)
    {
        app.active_turn_id = Some(id.to_string());
        app.set_status("turn started");
    }
}

fn handle_turn_completed(app: &mut AppState, params: &serde_json::Map<String, Value>) {
    app.active_turn_id = None;
    let keep_pending = app
        .approval
        .pending
        .as_ref()
        .map(PendingApprovalRequest::persists_after_turn_completed)
        .unwrap_or(false);
    if !keep_pending {
        app.clear_pending_approval();
    }
    let turn_status = params
        .get("turn")
        .and_then(Value::as_object)
        .and_then(|turn| turn.get("status"))
        .and_then(Value::as_str);
    let interrupted = turn_status == Some("interrupted");
    if interrupted {
        app.append_turn_interrupted_marker();
        app.set_status("turn interrupted");
    } else {
        app.set_status("turn completed");
    }
    app.handle_ralph_turn_completed(interrupted);
    if let Err(e) = app.apply_pending_ralph_toggle() {
        app.set_status(format!("ralph: {e}"));
    }
}

fn handle_codex_turn_diff(app: &mut AppState, params: &serde_json::Map<String, Value>) {
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
        app.upsert_turn_diff(turn_id, diff);
    }
}

#[cfg(test)]
pub(super) fn handle_notification_line(app: &mut AppState, line: &str) {
    let _ = handle_server_message_line(app, line);
}
