//! History loading helpers: reconstruct transcript from thread start/resume responses.

// --- Imports ---

use anyhow::Result;
use serde_json::Value;

use super::native_activity::upsert_native_activity;
use super::tool_diff::{command_execution_diff_output, extract_diff_blocks};
use super::tool_format::format_tool_item;
use super::tools::{is_tool_call_type, is_tool_output_type, item_text_from_content};
use super::{AppState, MessageKind, Role};
use crate::app::models::TurnStatus;
use crate::protocol_params::extract_result_object;

// --- Thread history reconstruction ---

pub(crate) fn load_history_from_start_or_resume(
    app: &mut AppState,
    response_line: &str,
) -> Result<()> {
    let parsed = extract_result_object(response_line)?;
    if let Some(thread_obj) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("thread"))
    {
        replace_history_from_thread(app, thread_obj);
    }
    Ok(())
}

pub(super) fn replace_history_from_thread(app: &mut AppState, thread_obj: &Value) {
    let thread_id = thread_obj
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(&app.thread_id)
        .to_string();
    let mut replacement = AppState::new(thread_id);
    append_history_from_thread(&mut replacement, thread_obj);
    app.replace_transcript_from(replacement);
    app.read_thread_name(thread_obj);
}

pub(super) fn append_history_from_thread(app: &mut AppState, thread_obj: &Value) {
    let Some(turns) = thread_obj.get("turns").and_then(Value::as_array) else {
        return;
    };

    for turn in turns {
        let status = turn
            .get("status")
            .and_then(Value::as_str)
            .and_then(TurnStatus::from_wire);
        if let (Some(turn_id), Some(status)) = (turn.get("id").and_then(Value::as_str), status) {
            app.record_historical_turn_status(turn_id, status);
        }
        let turn_is_terminal = status.is_some_and(TurnStatus::is_terminal);
        let Some(items) = turn.get("items").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            append_history_item(app, item);
            if item.get("type").and_then(Value::as_str) == Some("userMessage") {
                if let (Some(item_id), Some(turn_id)) = (
                    item.get("id").and_then(Value::as_str),
                    turn.get("id").and_then(Value::as_str),
                ) {
                    app.record_input_turn_id(item_id, turn_id);
                }
            }
            if turn_is_terminal || has_terminal_item_status(item) {
                if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                    app.mark_item_completed(item_id);
                }
            }
        }
    }
}

// --- History item helpers ---

fn append_history_item(app: &mut AppState, item: &Value) {
    if upsert_native_activity(app, item) {
        return;
    }
    let Some(kind) = item.get("type").and_then(Value::as_str) else {
        return;
    };
    match kind {
        "userMessage" => append_user_history_item(app, item),
        "agentMessage" => append_agent_history_item(app, item),
        "reasoning" => append_reasoning_history_item(app, item),
        "commandExecution" => append_tool_history_item(app, item, Role::ToolOutput),
        "contextCompaction" => append_context_compaction(app, item),
        kind if is_tool_call_type(kind) => append_tool_history_item(app, item, Role::ToolCall),
        kind if is_tool_output_type(kind) => append_tool_history_item(app, item, Role::ToolOutput),
        _ => {}
    }
}

fn append_user_history_item(app: &mut AppState, item: &Value) {
    let Some(text) = item_text_from_content(item) else {
        return;
    };
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        let idx = app.append_message(Role::User, text.clone());
        app.record_input_history(&text, Some(idx));
        return;
    };
    let inserted =
        app.upsert_mapped_message(item_id, Role::User, text.clone(), MessageKind::Plain, None);
    if inserted {
        let idx = app.item_to_message_index[item_id];
        app.record_input_history(&text, Some(idx));
    }
}

fn append_agent_history_item(app: &mut AppState, item: &Value) {
    let text = item
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| item_text_from_content(item));
    let Some(text) = text else {
        return;
    };
    let role = item
        .as_object()
        .map(agent_role_from_phase)
        .unwrap_or(Role::Assistant);
    upsert_history_message(app, item, role, text, MessageKind::Plain, None, 0);
}

fn append_reasoning_history_item(app: &mut AppState, item: &Value) {
    if let Some(text) = reasoning_summary_text(item) {
        upsert_history_message(
            app,
            item,
            Role::Reasoning,
            text,
            MessageKind::Plain,
            None,
            0,
        );
    }
}

fn append_context_compaction(app: &mut AppState, item: &Value) {
    upsert_history_message(
        app,
        item,
        Role::System,
        "↻ Context compacted".to_string(),
        MessageKind::Plain,
        None,
        0,
    );
}

fn append_tool_history_item(app: &mut AppState, item: &Value, role: Role) {
    let diffs = extract_diff_blocks(item);
    if !diffs.is_empty() {
        for (index, block) in diffs.into_iter().enumerate() {
            upsert_history_message(
                app,
                item,
                role,
                block.diff,
                MessageKind::Diff,
                block.file_path,
                index,
            );
        }
        return;
    }

    if role == Role::ToolOutput {
        if let Some(diff) = command_execution_diff_output(item) {
            upsert_history_message(app, item, role, diff, MessageKind::Diff, None, 0);
            return;
        }
    }

    if let Some(formatted) = format_tool_item(item, role) {
        if !formatted.is_empty() {
            upsert_history_message(app, item, role, formatted, MessageKind::Plain, None, 0);
            return;
        }
    }

    if let Some(t) = item.get("text").and_then(Value::as_str) {
        if !t.is_empty() {
            upsert_history_message(app, item, role, t.to_string(), MessageKind::Plain, None, 0);
            return;
        }
    }
    if let Some(text) = item_text_from_content(item) {
        upsert_history_message(app, item, role, text, MessageKind::Plain, None, 0);
    }
}

fn upsert_history_message(
    app: &mut AppState,
    item: &Value,
    role: Role,
    text: String,
    kind: MessageKind,
    file_path: Option<String>,
    part_index: usize,
) {
    let Some(item_id) = item.get("id").and_then(Value::as_str) else {
        if kind == MessageKind::Diff {
            app.append_diff_message(role, file_path, text);
        } else {
            app.append_message(role, text);
        }
        return;
    };
    let key = if part_index == 0 {
        item_id.to_string()
    } else {
        format!("{item_id}#part-{part_index}")
    };
    app.upsert_mapped_message(&key, role, text, kind, file_path);
}

fn has_terminal_item_status(item: &Value) -> bool {
    matches!(
        item.get("status").and_then(Value::as_str),
        Some("completed" | "failed" | "declined")
    )
}

pub(super) fn agent_role_from_phase(item: &serde_json::Map<String, Value>) -> Role {
    match item.get("phase").and_then(Value::as_str) {
        Some("commentary") => Role::Commentary,
        _ => Role::Assistant,
    }
}

// --- Reasoning summary ---

pub(super) fn reasoning_summary_text(item: &Value) -> Option<String> {
    let summary = item
        .get("summary")
        .or_else(|| item.get("summary_text"))?
        .as_array()?;
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
