//! Item-level notification handling: transcript mutation for tool calls, outputs, and history.

use serde_json::Value;

use super::item_history::{agent_role_from_phase, reasoning_summary_text};
use super::native_activity::upsert_native_activity;
use super::tools::{
    command_execution_diff_output, command_summary_from_shell_cmd, extract_diff_blocks,
    first_i64_at_paths, format_tool_item, is_tool_call_type, is_tool_output_type,
    item_text_from_content, role_for_tool_type, tool_command,
};
use super::transcript_source::source_path_from_item;
use super::{AppState, DiffBlock, MessageKind, Role};

// --- Item Notification Handling ---

pub(super) fn handle_item_notification(
    app: &mut AppState,
    method: &str,
    params: &serde_json::Map<String, Value>,
) -> bool {
    match method {
        "codex/event/raw_response_item" => true,
        "item/started" | "item/completed" => {
            let handled = if method == "item/started" {
                handle_item_started(app, params)
            } else {
                handle_item_completed(app, params)
            };
            record_user_turn_id(app, params);
            handled
        }
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
        "item/commandExecution/outputDelta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_command_output_delta(item_id, delta);
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

fn record_user_turn_id(app: &mut AppState, params: &serde_json::Map<String, Value>) {
    let Some(item) = params.get("item") else {
        return;
    };
    if item.get("type").and_then(Value::as_str) != Some("userMessage") {
        return;
    }
    if let (Some(item_id), Some(turn_id)) = (
        item.get("id").and_then(Value::as_str),
        params.get("turnId").and_then(Value::as_str),
    ) {
        app.record_input_turn_id(item_id, turn_id);
    }
}

fn handle_item_started(app: &mut AppState, params: &serde_json::Map<String, Value>) -> bool {
    let Some(item) = params.get("item").and_then(Value::as_object) else {
        return true;
    };
    let Some(t) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    if item
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|item_id| app.is_item_completed(item_id))
    {
        return true;
    }
    if matches!(t, "sleep" | "subAgentActivity" | "collabAgentToolCall")
        && upsert_native_activity(app, &Value::Object(item.clone()))
    {
        return true;
    }
    match t {
        "userMessage" => start_user_message(app, item),
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

fn start_user_message(app: &mut AppState, item: &serde_json::Map<String, Value>) {
    let Some(id) = item.get("id").and_then(Value::as_str) else {
        return;
    };
    let Some(text) = item_text_from_content(&Value::Object(item.clone())) else {
        return;
    };
    let inserted =
        app.upsert_mapped_message(id, Role::User, text.clone(), MessageKind::Plain, None);
    if inserted {
        let idx = app.item_to_message_index[id];
        app.record_input_history(&text, Some(idx));
    }
}

fn handle_item_completed(app: &mut AppState, params: &serde_json::Map<String, Value>) -> bool {
    let Some(item) = params.get("item").and_then(Value::as_object) else {
        return true;
    };
    let Some(kind) = item.get("type").and_then(Value::as_str) else {
        return true;
    };
    let item_value = Value::Object(item.clone());
    let handled = if upsert_native_activity(app, &item_value) {
        true
    } else {
        match kind {
            "userMessage" => complete_user_message(app, item, &item_value),
            "agentMessage" => complete_agent_message(app, item, &item_value),
            "reasoning" => complete_reasoning(app, item, &item_value),
            "contextCompaction" => complete_context_compaction(app, item),
            _ => complete_tool_item(app, item, kind, &item_value),
        }
    };
    if let Some(item_id) = item.get("id").and_then(Value::as_str) {
        app.mark_item_completed(item_id);
    }
    handled
}

fn complete_user_message(
    app: &mut AppState,
    item: &serde_json::Map<String, Value>,
    item_value: &Value,
) -> bool {
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        return true;
    };
    let text = item_text_from_content(item_value).unwrap_or_default();
    let inserted =
        app.upsert_mapped_message(item_id, Role::User, text.clone(), MessageKind::Plain, None);
    if inserted && !text.is_empty() {
        let idx = app.item_to_message_index[item_id];
        app.record_input_history(&text, Some(idx));
    }
    true
}

fn complete_context_compaction(app: &mut AppState, item: &serde_json::Map<String, Value>) -> bool {
    if let Some(item_id) = item.get("id").and_then(Value::as_str) {
        app.upsert_mapped_message(
            item_id,
            Role::System,
            "↻ Context compacted".to_string(),
            MessageKind::Plain,
            None,
        );
    }
    true
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
        app.upsert_mapped_message(id, role, text.unwrap_or_default(), MessageKind::Plain, None);
        app.maybe_disable_ralph_on_blocked_marker();
        return true;
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
    let text = reasoning_summary_text(item_value);
    if let Some(id) = item_id {
        app.upsert_mapped_message(
            id,
            Role::Reasoning,
            text.unwrap_or_default(),
            MessageKind::Plain,
            None,
        );
        return true;
    }
    if let Some(text) = text {
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
    let command_summary = item_id.and_then(|id| app.command_override(id)).or_else(|| {
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
            app.upsert_mapped_message(id, role, diff, MessageKind::Diff, None);
            return true;
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
            app.upsert_mapped_message(
                id,
                role,
                text,
                MessageKind::Plain,
                source_path_from_item(item_value),
            );
            return true;
        }
        let idx = app.append_message(role, text);
        app.messages[idx].file_path = source_path_from_item(item_value);
        return true;
    }
    if let Some(id) = item_id {
        app.upsert_mapped_message(id, role, String::new(), MessageKind::Plain, None);
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
        for (index, block) in diffs.into_iter().enumerate() {
            let key = if index == 0 {
                id.to_string()
            } else {
                format!("{id}#part-{index}")
            };
            app.upsert_mapped_message(&key, role, block.diff, MessageKind::Diff, block.file_path);
        }
        return true;
    }
    for block in diffs {
        app.append_diff_message(role, block.file_path, block.diff);
    }
    true
}
