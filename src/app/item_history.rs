//! History loading helpers: reconstruct transcript from thread start/resume responses.

// --- Imports ---

use anyhow::Result;
use serde_json::Value;

use super::native_activity::upsert_native_activity;
use super::{AppState, MessageKind, Role};
#[cfg(test)]
use crate::app::models::TurnStatus;
use crate::protocol_params::extract_result_object;
use crate::session::history_identity::HistoryMode;

#[path = "item_history_draft.rs"]
pub(super) mod history_draft;

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
        replace_history_from_thread(app, thread_obj)?;
    }
    Ok(())
}

pub(super) fn replace_history_from_thread(app: &mut AppState, thread_obj: &Value) -> Result<()> {
    let thread_id = thread_obj
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(app.session.thread_id());
    anyhow::ensure!(
        thread_id == app.session.thread_id(),
        "History belongs to another thread"
    );
    let previous = app.session.replace_codex_item_turn(None);
    let draft = history_draft::build(app, thread_obj);
    app.replace_history_draft(draft, HistoryMode::SameThread);
    app.session.replace_codex_item_turn(previous);
    app.read_thread_name(thread_obj);
    Ok(())
}

#[cfg(test)]
pub(super) fn append_history_from_thread(app: &mut AppState, thread_obj: &Value) {
    let Some(turns) = thread_obj.get("turns").and_then(Value::as_array) else {
        return;
    };

    let previous = app.session.replace_codex_item_turn(None);
    for turn in turns {
        app.session
            .replace_codex_item_turn(turn["id"].as_str().map(str::to_string));
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
    app.session.replace_codex_item_turn(previous);
}

// --- History item helpers ---

pub(super) fn append_history_item(app: &mut AppState, item: &Value) {
    if upsert_native_activity(app, item) {
        return;
    }
    let rows = history_draft::rows(app, item);
    for (part, row) in rows.into_iter().enumerate() {
        let user = row.role == Role::User;
        let describe = row.role == Role::ToolOutput || row.kind == MessageKind::Diff;
        let text = row.text.clone();
        if let Some(id) = item["id"].as_str() {
            app.with_item_part(id, part, |app, key| {
                let inserted =
                    app.upsert_mapped_message(key, row.role, row.text, row.kind, row.file_path);
                let index = app.item_message_index(key).expect("native history row");
                if user && inserted {
                    app.record_input_history(&text, Some(index));
                }
                if describe {
                    app.describe_tool_output(index, item);
                }
            });
        } else {
            let index = app.append_draft_message(row);
            if user {
                app.record_input_history(&text, Some(index));
            }
            if describe {
                app.describe_tool_output(index, item);
            }
        }
    }
    app.observe_summary_item(item, false);
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
