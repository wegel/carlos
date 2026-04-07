//! History loading helpers: reconstruct transcript from thread start/resume responses.

use anyhow::Result;
use serde_json::Value;

use super::tools::*;
use super::{AppState, Role};
use crate::protocol_params::extract_result_object;

pub(super) fn append_item_text_from_content(app: &mut AppState, item: &Value, role: Role) {
    if let Some(text) = item_text_from_content(item) {
        app.append_message(role, text);
    }
}

pub(super) fn append_tool_history_item(app: &mut AppState, item: &Value, role: Role) {
    let diffs = extract_diff_blocks(item);
    if !diffs.is_empty() {
        for block in diffs {
            app.append_diff_message(role, block.file_path, block.diff);
        }
        return;
    }

    if role == Role::ToolOutput {
        if let Some(diff) = command_execution_diff_output(item) {
            app.append_diff_message(role, None, diff);
            return;
        }
    }

    if let Some(formatted) = format_tool_item(item, role) {
        if !formatted.is_empty() {
            app.append_message(role, formatted);
            return;
        }
    }

    if let Some(t) = item.get("text").and_then(Value::as_str) {
        if !t.is_empty() {
            app.append_message(role, t.to_string());
            return;
        }
    }
    append_item_text_from_content(app, item, role);
}

pub(super) fn agent_role_from_phase(item: &serde_json::Map<String, Value>) -> Role {
    match item.get("phase").and_then(Value::as_str) {
        Some("commentary") => Role::Commentary,
        _ => Role::Assistant,
    }
}

pub(super) fn append_history_from_thread(app: &mut AppState, thread_obj: &Value) {
    let Some(turns) = thread_obj.get("turns").and_then(Value::as_array) else {
        return;
    };

    for turn in turns {
        let Some(items) = turn.get("items").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            let Some(kind) = item.get("type").and_then(Value::as_str) else {
                continue;
            };

            match kind {
                "userMessage" => {
                    if let Some(text) = item_text_from_content(item) {
                        let idx = app.append_message(Role::User, text.clone());
                        app.record_input_history(&text, Some(idx));
                    }
                }
                "agentMessage" => {
                    if let Some(t) = item.get("text").and_then(Value::as_str) {
                        let role = item
                            .as_object()
                            .map(agent_role_from_phase)
                            .unwrap_or(Role::Assistant);
                        app.append_message(role, t.to_string());
                    }
                }
                "reasoning" => {
                    if let Some(text) = reasoning_summary_text(item) {
                        app.append_message(Role::Reasoning, text);
                    }
                }
                "commandExecution" => {
                    append_tool_history_item(app, item, Role::ToolOutput);
                }
                "contextCompaction" => {
                    app.append_context_compacted_marker();
                }
                k if is_tool_call_type(k) => {
                    append_tool_history_item(app, item, Role::ToolCall);
                }
                k if is_tool_output_type(k) => {
                    append_tool_history_item(app, item, Role::ToolOutput);
                }
                _ => {}
            }
        }
    }
}

pub(super) fn load_history_from_start_or_resume(
    app: &mut AppState,
    response_line: &str,
) -> Result<()> {
    let parsed = extract_result_object(response_line)?;
    if let Some(thread_obj) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("thread"))
    {
        append_history_from_thread(app, thread_obj);
    }
    Ok(())
}

pub(super) fn reasoning_summary_text(item: &Value) -> Option<String> {
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
