//! Replay captured offline source items through the existing core text/grouping rules.

use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::tools::{
    extract_diff_blocks, format_tool_item, raw_function_call_output_to_tool_item,
    raw_function_call_to_tool_item,
};
use serde_json::Value;

pub(super) fn append_perf_session_item(session: &mut SessionState, payload: &Value) -> bool {
    let Some(kind) = payload.get("type").and_then(Value::as_str) else {
        return false;
    };
    match kind {
        "message" => append_perf_message(session, payload),
        "reasoning" => append_perf_reasoning(session, payload),
        "function_call" => append_perf_function_call(session, payload),
        "function_call_output" => append_perf_function_call_output(session, payload),
        _ => false,
    }
}

fn append_perf_message(session: &mut SessionState, payload: &Value) -> bool {
    let role = match payload.get("role").and_then(Value::as_str) {
        Some("user") => Role::User,
        Some("assistant") => {
            if payload.get("phase").and_then(Value::as_str) == Some("commentary") {
                Role::Commentary
            } else {
                Role::Assistant
            }
        }
        _ => return false,
    };
    let text = payload
        .get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| session_message_text(payload));
    if let Some(text) = text.filter(|text| !text.trim().is_empty()) {
        session.append_message(role, text);
        return true;
    }
    false
}

fn append_perf_reasoning(session: &mut SessionState, payload: &Value) -> bool {
    let Some(text) = reasoning_summary_text(payload) else {
        return false;
    };
    session.append_message(Role::Reasoning, text);
    true
}

fn append_perf_function_call(session: &mut SessionState, payload: &Value) -> bool {
    let Some((_, tool_item)) = raw_function_call_to_tool_item(payload) else {
        return false;
    };
    let Some(text) = format_tool_item(&tool_item, Role::ToolCall) else {
        return false;
    };
    if text.trim().is_empty() {
        return false;
    }
    session.append_message(Role::ToolCall, text);
    true
}

fn append_perf_function_call_output(session: &mut SessionState, payload: &Value) -> bool {
    let Some((_, tool_item)) = raw_function_call_output_to_tool_item(payload) else {
        return false;
    };
    let diffs = extract_diff_blocks(&tool_item);
    if let Some(first) = diffs.first() {
        session.append_diff_message(
            Role::ToolOutput,
            first.file_path.clone(),
            first.diff.clone(),
        );
        for block in diffs.iter().skip(1) {
            session.append_diff_message(
                Role::ToolOutput,
                block.file_path.clone(),
                block.diff.clone(),
            );
        }
        return true;
    }
    let Some(text) = format_tool_item(&tool_item, Role::ToolOutput) else {
        return false;
    };
    if text.trim().is_empty() {
        return false;
    }
    session.append_message(Role::ToolOutput, text);
    true
}

// --- Utilities ---

fn reasoning_summary_text(item: &Value) -> Option<String> {
    let summary = item.get("summary")?.as_array()?;
    let mut parts = Vec::new();
    for entry in summary {
        if let Some(text) = entry.as_str() {
            if !text.trim().is_empty() {
                parts.push(text.to_string());
            }
            continue;
        }

        let text = entry.get("text").and_then(Value::as_str).or_else(|| {
            entry
                .get("content")
                .and_then(Value::as_array)
                .and_then(|parts| parts.first())
                .and_then(|part| part.get("text"))
                .and_then(Value::as_str)
        });
        if let Some(text) = text.filter(|t| !t.trim().is_empty()) {
            parts.push(text.to_string());
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn session_message_text(item: &Value) -> Option<String> {
    let content = item.get("content").and_then(Value::as_array)?;
    let mut parts = Vec::new();
    for part in content {
        let Some(kind) = part.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(kind, "text" | "output_text" | "input_text") {
            continue;
        }
        if let Some(text) = part.get("text").and_then(Value::as_str) {
            if !text.is_empty() {
                parts.push(text.to_string());
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}
