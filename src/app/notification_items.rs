//! Item-level notification handling: transcript mutation for tool calls, outputs, and history.

use serde_json::Value;

use super::item_history::{agent_role_from_phase, reasoning_summary_text};
use super::tools::{
    command_execution_diff_output, command_summary_from_shell_cmd, extract_diff_blocks,
    first_i64_at_paths, format_tool_item, is_tool_call_type, is_tool_output_type,
    item_text_from_content, raw_function_call_output_to_tool_item, raw_function_call_to_tool_item,
    role_for_tool_type, tool_command,
};
use super::{AppState, DiffBlock, MessageKind, Role};

pub(super) fn upsert_tool_message(
    app: &mut AppState,
    key: &str,
    role: Role,
    text: String,
    kind: MessageKind,
    file_path: Option<String>,
) {
    app.upsert_mapped_message(key, role, text, kind, file_path);
}

pub(super) fn handle_raw_response_item(app: &mut AppState, item: &Value) {
    if let Some((call_id, tool_item)) = raw_function_call_to_tool_item(item) {
        if app.has_agent_item_mapping(&call_id) {
            return;
        }
        if let Some(formatted) = format_tool_item(&tool_item, Role::ToolCall) {
            if !formatted.trim().is_empty() {
                upsert_tool_message(
                    app,
                    &call_id,
                    Role::ToolCall,
                    formatted,
                    MessageKind::Plain,
                    None,
                );
            }
        }
        return;
    }

    if let Some((call_id, tool_item)) = raw_function_call_output_to_tool_item(item) {
        let diffs = extract_diff_blocks(&tool_item);
        if let Some(first) = diffs.first() {
            upsert_tool_message(
                app,
                &call_id,
                Role::ToolOutput,
                first.diff.clone(),
                MessageKind::Diff,
                first.file_path.clone(),
            );
            for block in diffs.iter().skip(1) {
                app.append_diff_message(
                    Role::ToolOutput,
                    block.file_path.clone(),
                    block.diff.clone(),
                );
            }
            return;
        }

        if let Some(formatted) = format_tool_item(&tool_item, Role::ToolOutput) {
            if !formatted.trim().is_empty() {
                upsert_tool_message(
                    app,
                    &call_id,
                    Role::ToolOutput,
                    formatted,
                    MessageKind::Plain,
                    None,
                );
            }
        }
    }
}

// --- Item Notification Handling ---

pub(super) fn handle_item_notification(
    app: &mut AppState,
    method: &str,
    params: &serde_json::Map<String, Value>,
) -> bool {
    match method {
        "codex/event/raw_response_item" => {
            if let Some(item) = params.get("msg").and_then(|m| m.get("item")) {
                handle_raw_response_item(app, item);
            }
            true
        }
        "item/started" => handle_item_started(app, params),
        "item/completed" => handle_item_completed(app, params),
        "item/agentMessage/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
                app.maybe_disable_ralph_on_blocked_marker();
            }
            true
        }
        "item/reasoning/summaryTextDelta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_reasoning_summary_delta(item_id, delta);
            }
            true
        }
        "item/toolCall/delta"
        | "item/tool_call/delta"
        | "item/toolInvocation/delta"
        | "item/functionCall/delta"
        | "item/mcpToolCall/delta"
        | "item/toolResult/delta"
        | "item/toolOutput/delta"
        | "item/tool_result/delta"
        | "item/functionCallOutput/delta"
        | "item/mcpToolResult/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
            }
            true
        }
        _ => false,
    }
}

fn handle_item_started(
    app: &mut AppState,
    params: &serde_json::Map<String, Value>,
) -> bool {
    let Some(item) = params.get("item").and_then(Value::as_object) else {
        return true;
    };
    let Some(t) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    match t {
        "userMessage" => {
            let item_value = Value::Object(item.clone());
            if let Some(text) = item_text_from_content(&item_value) {
                let idx = app.append_message(Role::User, text.clone());
                app.record_input_history(&text, Some(idx));
            }
        }
        "agentMessage" => {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                let role = agent_role_from_phase(item);
                app.ensure_item_placeholder(id, role);
            }
        }
        "reasoning" => {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                app.ensure_item_placeholder(id, Role::Reasoning);
            }
        }
        "commandExecution" => {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                app.ensure_item_placeholder(id, Role::ToolCall);
            }
        }
        t if is_tool_call_type(t) => {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                app.ensure_item_placeholder(id, Role::ToolCall);
            }
        }
        t if is_tool_output_type(t) => {
            if let Some(id) = item.get("id").and_then(Value::as_str) {
                app.ensure_item_placeholder(id, Role::ToolOutput);
            }
        }
        _ => {}
    }
    true
}

fn handle_item_completed(
    app: &mut AppState,
    params: &serde_json::Map<String, Value>,
) -> bool {
    let Some(item) = params.get("item").and_then(Value::as_object) else {
        return true;
    };
    let Some(kind) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    let item_value = Value::Object(item.clone());

    if kind == "contextCompaction" {
        app.append_context_compacted_marker();
        return true;
    }
    if kind == "agentMessage" {
        return complete_agent_message(app, item, &item_value);
    }
    if kind == "reasoning" {
        return complete_reasoning(app, item, &item_value);
    }
    complete_tool_item(app, item, kind, &item_value)
}

fn complete_agent_message(
    app: &mut AppState,
    item: &serde_json::Map<String, Value>,
    item_value: &Value,
) -> bool {
    let role = agent_role_from_phase(item);
    let item_id = item.get("id").and_then(Value::as_str);
    let text = item
        .get("text")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| item_text_from_content(item_value));
    if let Some(id) = item_id {
        if app.update_mapped_message(id, role, text.clone(), MessageKind::Plain, None) {
            app.maybe_disable_ralph_on_blocked_marker();
            return true;
        }
    }
    if let Some(text) = text {
        app.append_message(role, text);
        app.maybe_disable_ralph_on_blocked_marker();
    }
    true
}

fn complete_reasoning(
    app: &mut AppState,
    item: &serde_json::Map<String, Value>,
    item_value: &Value,
) -> bool {
    let item_id = item.get("id").and_then(Value::as_str);
    if let Some(text) = reasoning_summary_text(item_value) {
        if let Some(id) = item_id {
            if app.update_mapped_message(
                id,
                Role::Reasoning,
                Some(text.clone()),
                MessageKind::Plain,
                None,
            ) {
                return true;
            }
        }
        app.append_message(Role::Reasoning, text);
    }
    true
}

fn complete_tool_item(
    app: &mut AppState,
    item: &serde_json::Map<String, Value>,
    kind: &str,
    item_value: &Value,
) -> bool {
    let Some(mut role) = role_for_tool_type(kind) else {
        return true;
    };
    if kind == "commandExecution" {
        role = Role::ToolOutput;
    }

    let diffs = extract_diff_blocks(item_value);
    if diffs.is_empty() {
        return complete_tool_item_no_diff(app, item, role, item_value);
    }
    complete_tool_item_with_diffs(app, item, role, diffs)
}

fn complete_tool_item_no_diff(
    app: &mut AppState,
    item: &serde_json::Map<String, Value>,
    role: Role,
    item_value: &Value,
) -> bool {
    let item_id = item.get("id").and_then(Value::as_str);
    let exit_code = first_i64_at_paths(item_value, &[&["exitCode"], &["exit_code"]]);
    let command_summary = item_id
        .and_then(|id| app.command_override(id))
        .or_else(|| {
            tool_command(item_value).and_then(|cmd| command_summary_from_shell_cmd(&cmd, None))
        });

    if let (Some(id), Some(summary)) = (item_id, command_summary.clone()) {
        if exit_code.unwrap_or(0) == 0 {
            app.upsert_mapped_message(id, Role::ToolCall, summary, MessageKind::Plain, None);
            return true;
        }
    }

    if let Some(diff) = command_execution_diff_output(item_value) {
        if let Some(id) = item_id {
            if app.update_mapped_message(id, role, Some(diff.clone()), MessageKind::Diff, None) {
                return true;
            }
        }
        app.append_diff_message(role, None, diff);
        return true;
    }

    if let Some(formatted) = format_tool_item(item_value, role) {
        let text = if exit_code.unwrap_or(0) != 0 {
            if let Some(summary) = command_summary {
                format!("{summary}\n{formatted}")
            } else {
                formatted
            }
        } else {
            formatted
        };
        if let Some(id) = item_id {
            if app.update_mapped_message(id, role, Some(text.clone()), MessageKind::Plain, None) {
                return true;
            }
        }
        app.append_message(role, text);
    }
    true
}

fn complete_tool_item_with_diffs(
    app: &mut AppState,
    item: &serde_json::Map<String, Value>,
    role: Role,
    diffs: Vec<DiffBlock>,
) -> bool {
    let item_id = item.get("id").and_then(Value::as_str);
    if let Some(id) = item_id {
        if let Some(first) = diffs.first() {
            if app.update_mapped_message(
                id,
                role,
                Some(first.diff.clone()),
                MessageKind::Diff,
                first.file_path.clone(),
            ) {
                for block in diffs.iter().skip(1) {
                    app.append_diff_message(role, block.file_path.clone(), block.diff.clone());
                }
                return true;
            }
        }
    }
    for block in diffs {
        app.append_diff_message(role, block.file_path, block.diff);
    }
    true
}
