use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
use serde_json::Value;

use super::context_usage::{
    context_usage_from_thread_token_usage_params, context_usage_from_token_count_params,
};
use super::tools::*;
use super::{AppState, MessageKind, Role, ThreadSummary};
use crate::protocol::*;
use crate::theme::KITT_STEP_MS;

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

pub(super) fn upsert_tool_message(
    app: &mut AppState,
    key: &str,
    role: Role,
    text: String,
    kind: MessageKind,
    file_path: Option<String>,
) {
    if let Some(idx) = app.agent_item_to_index.get(key).copied() {
        if let Some(msg) = app.messages.get_mut(idx) {
            msg.role = role;
            msg.text = text;
            msg.kind = kind;
            msg.file_path = file_path;
            app.auto_follow_bottom = true;
            app.mark_transcript_dirty();
            return;
        }
    }

    let idx = if kind == MessageKind::Diff {
        app.append_diff_message(role, file_path, text)
    } else {
        app.append_message(role, text)
    };
    app.put_agent_item_mapping(key, idx);
    app.auto_follow_bottom = true;
}

pub(super) fn handle_raw_response_item(app: &mut AppState, item: &Value) {
    if let Some((call_id, tool_item)) = raw_function_call_to_tool_item(item) {
        if app.agent_item_to_index.contains_key(&call_id) {
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
            app.auto_follow_bottom = true;
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
                        app.append_message(Role::Assistant, t.to_string());
                    }
                }
                "reasoning" => {
                    let Some(summary) = item.get("summary").and_then(Value::as_array) else {
                        continue;
                    };
                    let mut parts = Vec::new();
                    for s in summary {
                        if let Some(t) = s.as_str() {
                            parts.push(t);
                        }
                    }
                    if !parts.is_empty() {
                        app.append_message(Role::Reasoning, parts.join("\n"));
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

pub(super) fn parse_thread_list(response_line: &str) -> Result<Vec<ThreadSummary>> {
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
        let preview = obj.get("preview").and_then(Value::as_str).unwrap_or("");
        let cwd = obj.get("cwd").and_then(Value::as_str).unwrap_or("");
        let updated_at = obj
            .get("updatedAt")
            .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|n| n as i64)))
            .unwrap_or(0);

        out.push(ThreadSummary {
            id: id.to_string(),
            preview: preview.to_string(),
            cwd: cwd.to_string(),
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

pub(super) fn animation_tick() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() / KITT_STEP_MS)
        .unwrap_or(0)
}

pub(super) fn animation_poll_timeout(working: bool) -> Duration {
    if !working {
        return Duration::from_millis(25);
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let rem = KITT_STEP_MS - (now_ms % KITT_STEP_MS);
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

pub(super) fn handle_notification_line(app: &mut AppState, line: &str) {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return;
    };
    let Some(params) = parsed.get("params").and_then(Value::as_object) else {
        return;
    };

    match method {
        "thread/tokenUsage/updated" => {
            if let Some(usage) = context_usage_from_thread_token_usage_params(params) {
                app.context_usage = Some(usage);
            }
        }
        "thread/compacted" => {
            app.append_context_compacted_marker();
        }
        "turn/started" => {
            app.context_usage = None;
            if let Some(id) = params
                .get("turn")
                .and_then(Value::as_object)
                .and_then(|t| t.get("id"))
                .and_then(Value::as_str)
            {
                app.active_turn_id = Some(id.to_string());
                app.auto_follow_bottom = true;
                app.set_status("turn started");
            }
        }
        "turn/completed" => {
            app.active_turn_id = None;
            let turn_status = params
                .get("turn")
                .and_then(Value::as_object)
                .and_then(|turn| turn.get("status"))
                .and_then(Value::as_str);
            if turn_status == Some("interrupted") {
                app.append_turn_interrupted_marker();
                app.set_status("turn interrupted");
            } else {
                app.set_status("turn completed");
            }
        }
        "turn/diff/updated" => {
            if let (Some(turn_id), Some(diff)) = (
                params.get("turnId").and_then(Value::as_str),
                params.get("diff").and_then(Value::as_str),
            ) {
                app.upsert_turn_diff(turn_id, diff);
            }
        }
        "codex/event/turn_diff" => {
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
        "codex/event/raw_response_item" => {
            if let Some(item) = params.get("msg").and_then(|m| m.get("item")) {
                handle_raw_response_item(app, item);
            }
        }
        "item/started" => {
            let Some(item) = params.get("item").and_then(Value::as_object) else {
                return;
            };
            let Some(t) = item.get("type").and_then(Value::as_str) else {
                return;
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
                        let idx = app.append_message(Role::Assistant, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                "reasoning" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        let idx = app.append_message(Role::Reasoning, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                "commandExecution" => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        if app.agent_item_to_index.contains_key(id) {
                            return;
                        }
                        let idx = app.append_message(Role::ToolCall, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                t if is_tool_call_type(t) => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        if app.agent_item_to_index.contains_key(id) {
                            return;
                        }
                        let idx = app.append_message(Role::ToolCall, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                t if is_tool_output_type(t) => {
                    if let Some(id) = item.get("id").and_then(Value::as_str) {
                        if app.agent_item_to_index.contains_key(id) {
                            return;
                        }
                        let idx = app.append_message(Role::ToolOutput, String::new());
                        app.put_agent_item_mapping(id, idx);
                    }
                }
                _ => {}
            }
        }
        "item/completed" => {
            let Some(item) = params.get("item").and_then(Value::as_object) else {
                return;
            };
            let Some(kind) = item.get("type").and_then(Value::as_str) else {
                return;
            };
            if kind == "contextCompaction" {
                app.append_context_compacted_marker();
                return;
            }
            let Some(mut role) = role_for_tool_type(kind) else {
                return;
            };
            if kind == "commandExecution" {
                role = Role::ToolOutput;
            }

            let item_value = Value::Object(item.clone());
            let diffs = extract_diff_blocks(&item_value);
            if diffs.is_empty() {
                let item_id = item.get("id").and_then(Value::as_str);
                let exit_code = first_i64_at_paths(&item_value, &[&["exitCode"], &["exit_code"]]);
                let summary_override =
                    item_id.and_then(|id| app.command_render_overrides.get(id).cloned());
                if let (Some(id), Some(summary)) = (item_id, summary_override.clone()) {
                    if exit_code.unwrap_or(0) == 0 {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = Role::ToolCall;
                                msg.text = summary;
                                msg.kind = MessageKind::Plain;
                                msg.file_path = None;
                            }
                            app.mark_transcript_dirty();
                            app.auto_follow_bottom = true;
                            return;
                        }
                        app.append_message(Role::ToolCall, summary);
                        app.auto_follow_bottom = true;
                        return;
                    }
                }

                if let Some(diff) = command_execution_diff_output(&item_value) {
                    if let Some(id) = item_id {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = role;
                                msg.text = diff;
                                msg.kind = MessageKind::Diff;
                                msg.file_path = None;
                            }
                            app.mark_transcript_dirty();
                            app.auto_follow_bottom = true;
                            return;
                        }
                    }
                    app.append_diff_message(role, None, diff);
                    app.auto_follow_bottom = true;
                    return;
                }

                if let Some(formatted) = format_tool_item(&item_value, role) {
                    let text = if exit_code.unwrap_or(0) != 0 {
                        if let Some(summary) = summary_override {
                            format!("{summary}\n{formatted}")
                        } else {
                            formatted
                        }
                    } else {
                        formatted
                    };
                    let item_id = item.get("id").and_then(Value::as_str);
                    if let Some(id) = item_id {
                        if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                            if let Some(msg) = app.messages.get_mut(idx) {
                                msg.role = role;
                                msg.text = text;
                                msg.kind = MessageKind::Plain;
                                msg.file_path = None;
                            }
                            app.mark_transcript_dirty();
                            app.auto_follow_bottom = true;
                            return;
                        }
                    }
                    app.append_message(role, text);
                    app.auto_follow_bottom = true;
                }
                return;
            }

            let item_id = item.get("id").and_then(Value::as_str);
            if let Some(id) = item_id {
                if let Some(idx) = app.agent_item_to_index.get(id).copied() {
                    if let Some(first) = diffs.first() {
                        if let Some(msg) = app.messages.get_mut(idx) {
                            msg.role = role;
                            msg.text = first.diff.clone();
                            msg.kind = MessageKind::Diff;
                            msg.file_path = first.file_path.clone();
                        }
                        app.mark_transcript_dirty();
                        for block in diffs.iter().skip(1) {
                            app.append_diff_message(
                                role,
                                block.file_path.clone(),
                                block.diff.clone(),
                            );
                        }
                        app.auto_follow_bottom = true;
                        return;
                    }
                }
            }

            for block in diffs {
                app.append_diff_message(role, block.file_path, block.diff);
            }
            app.auto_follow_bottom = true;
        }
        "item/agentMessage/delta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
                app.auto_follow_bottom = true;
            }
        }
        "item/reasoning/summaryTextDelta" => {
            if let (Some(item_id), Some(delta)) = (
                params.get("itemId").and_then(Value::as_str),
                params.get("delta").and_then(Value::as_str),
            ) {
                app.upsert_agent_delta(item_id, delta);
            }
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
                app.auto_follow_bottom = true;
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
