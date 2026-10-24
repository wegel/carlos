//! Item-level notification handling: transcript mutation for tool calls, outputs, and history.

use serde_json::Value;

use super::item_history::{agent_role_from_phase, reasoning_summary_text};
use super::native_activity::upsert_native_activity;
use super::notification_tools::complete_tool_item;
use super::tools::{is_tool_call_type, is_tool_output_type, item_text_from_content};
use super::{AppState, MessageKind, Role};

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
                if method.contains("Result/")
                    || method.contains("Output/")
                    || method.contains("tool_result/")
                {
                    app.upsert_command_output_delta(item_id, delta);
                } else {
                    app.upsert_agent_delta(item_id, delta);
                }
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
                let idx = app.item_to_message_index[id];
                app.describe_tool_output(idx, &Value::Object(item.clone()));
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
                let idx = app.item_to_message_index[id];
                app.describe_tool_output(idx, &Value::Object(item.clone()));
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
            _ => complete_tool_item(app, kind, &item_value),
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
