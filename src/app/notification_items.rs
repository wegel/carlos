use anyhow::Result;
use serde_json::Value;

use super::tools::*;
use super::{AppState, MessageKind, Role};
use crate::protocol::extract_result_object;

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

fn agent_role_from_phase(item: &serde_json::Map<String, Value>) -> Role {
    match item.get("phase").and_then(Value::as_str) {
        Some("commentary") => Role::Commentary,
        _ => Role::Assistant,
    }
}

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
        "item/started" => {
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
        "item/completed" => {
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
                let role = agent_role_from_phase(item);
                let item_id = item.get("id").and_then(Value::as_str);
                let text = item
                    .get("text")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .or_else(|| item_text_from_content(&item_value));
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
                return true;
            }

            if kind == "reasoning" {
                let item_id = item.get("id").and_then(Value::as_str);
                if let Some(text) = reasoning_summary_text(&item_value) {
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
                return true;
            }

            let Some(mut role) = role_for_tool_type(kind) else {
                return true;
            };
            if kind == "commandExecution" {
                role = Role::ToolOutput;
            }

            let diffs = extract_diff_blocks(&item_value);
            if diffs.is_empty() {
                let item_id = item.get("id").and_then(Value::as_str);
                let exit_code = first_i64_at_paths(&item_value, &[&["exitCode"], &["exit_code"]]);
                let command_summary = item_id
                    .and_then(|id| app.command_override(id))
                    .or_else(|| {
                        tool_command(&item_value)
                            .and_then(|cmd| command_summary_from_shell_cmd(&cmd, None))
                    });
                if let (Some(id), Some(summary)) = (item_id, command_summary.clone()) {
                    if exit_code.unwrap_or(0) == 0 {
                        app.upsert_mapped_message(
                            id,
                            Role::ToolCall,
                            summary,
                            MessageKind::Plain,
                            None,
                        );
                        return true;
                    }
                }

                if let Some(diff) = command_execution_diff_output(&item_value) {
                    if let Some(id) = item_id {
                        if app.update_mapped_message(
                            id,
                            role,
                            Some(diff.clone()),
                            MessageKind::Diff,
                            None,
                        ) {
                            return true;
                        }
                    }
                    app.append_diff_message(role, None, diff);
                    return true;
                }

                if let Some(formatted) = format_tool_item(&item_value, role) {
                    let text = if exit_code.unwrap_or(0) != 0 {
                        if let Some(summary) = command_summary {
                            format!("{summary}\n{formatted}")
                        } else {
                            formatted
                        }
                    } else {
                        formatted
                    };
                    let item_id = item.get("id").and_then(Value::as_str);
                    if let Some(id) = item_id {
                        if app.update_mapped_message(
                            id,
                            role,
                            Some(text.clone()),
                            MessageKind::Plain,
                            None,
                        ) {
                            return true;
                        }
                    }
                    app.append_message(role, text);
                }
                return true;
            }

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
                            app.append_diff_message(
                                role,
                                block.file_path.clone(),
                                block.diff.clone(),
                            );
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
