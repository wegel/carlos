//! History loading helpers: reconstruct transcript from thread start/resume responses.

// --- Imports ---

use anyhow::Result;
use serde_json::Value;

use super::item_changes::ItemChanges;
use super::item_receive::{receive_summary, record_prompt, upsert_native_activity};
#[cfg(test)]
use super::models::TurnStatus;
use super::models::{MessageKind, Role};
use super::state::SessionState;
use crate::protocol_params::extract_result_object;
use crate::session::history_identity::HistoryMode;

use super::item_history_draft as history_draft;

// --- Thread history reconstruction ---

/// Adopt a checked start/resume history and report its transcript edits.
pub(crate) fn load_history_from_start_or_resume(
    session: &mut SessionState,
    response_line: &str,
) -> Result<ItemChanges> {
    let parsed = extract_result_object(response_line)?;
    if let Some(thread_obj) = parsed
        .get("result")
        .and_then(Value::as_object)
        .and_then(|r| r.get("thread"))
    {
        return replace_history_from_thread(session, thread_obj);
    }
    Ok(ItemChanges::default())
}

/// Reject another thread before building and adopting a complete native history.
pub(crate) fn replace_history_from_thread(
    session: &mut SessionState,
    thread_obj: &Value,
) -> Result<ItemChanges> {
    let thread_id = thread_obj
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or(session.thread_id());
    anyhow::ensure!(
        thread_id == session.thread_id(),
        "History belongs to another thread"
    );
    let previous = session.replace_codex_item_turn(None);
    let draft = history_draft::build(session, thread_obj);
    let changes = session.replace_native_history(draft, HistoryMode::SameThread);
    session.replace_codex_item_turn(previous);
    session.observe_thread_name(thread_obj);
    Ok(changes)
}

#[cfg(test)]
pub(crate) fn append_history_from_thread(
    session: &mut SessionState,
    thread_obj: &Value,
) -> ItemChanges {
    let mut changes = ItemChanges::default();
    let Some(turns) = thread_obj.get("turns").and_then(Value::as_array) else {
        return changes;
    };

    let previous = session.replace_codex_item_turn(None);
    for turn in turns {
        session.replace_codex_item_turn(turn["id"].as_str().map(str::to_string));
        let status = turn
            .get("status")
            .and_then(Value::as_str)
            .and_then(TurnStatus::from_wire);
        if let (Some(turn_id), Some(status)) = (turn.get("id").and_then(Value::as_str), status) {
            session.record_historical_turn_status(turn_id, status);
        }
        let turn_is_terminal = status.is_some_and(TurnStatus::is_terminal);
        let Some(items) = turn.get("items").and_then(Value::as_array) else {
            continue;
        };
        for item in items {
            append_history_item(session, &mut changes, item);
            if item.get("type").and_then(Value::as_str) == Some("userMessage") {
                if let (Some(item_id), Some(turn_id)) = (
                    item.get("id").and_then(Value::as_str),
                    turn.get("id").and_then(Value::as_str),
                ) {
                    super::item_receive::record_native_turn(
                        session,
                        &mut changes,
                        item_id,
                        turn_id,
                    );
                }
            }
            if turn_is_terminal || has_terminal_item_status(item) {
                if let Some(item_id) = item.get("id").and_then(Value::as_str) {
                    session.mark_item_completed(&session.backend_entry_key(item_id));
                }
            }
        }
    }
    session.replace_codex_item_turn(previous);
    changes
}

// --- History item helpers ---

pub(crate) fn append_history_item(
    session: &mut SessionState,
    changes: &mut ItemChanges,
    item: &Value,
) {
    if upsert_native_activity(session, changes, item) {
        return;
    }
    let rows = history_draft::rows(session, item);
    for (part, row) in rows.into_iter().enumerate() {
        let user = row.role == Role::User;
        let describe = row.role == Role::ToolOutput || row.kind == MessageKind::Diff;
        let text = row.text.clone();
        if let Some(id) = item["id"].as_str() {
            session.with_native_item_part(id, part, |session, key| {
                let inserted = changes.track(session.upsert_mapped_message(
                    &session.backend_entry_key(key),
                    row.role,
                    row.text,
                    row.kind,
                    row.file_path,
                ));
                let index = session
                    .item_message_index(&session.backend_entry_key(key))
                    .expect("native history row");
                if user && inserted {
                    record_prompt(session, changes, &text, Some(index));
                }
                if describe {
                    super::item_receive::describe_output(session, changes, index, item);
                }
            });
        } else {
            let index = changes.track(session.append_draft_message(row));
            if user {
                record_prompt(session, changes, &text, Some(index));
            }
            if describe {
                super::item_receive::describe_output(session, changes, index, item);
            }
        }
    }
    receive_summary(session, changes, item, false);
}

pub(super) fn has_terminal_item_status(item: &Value) -> bool {
    matches!(
        item.get("status").and_then(Value::as_str),
        Some("completed" | "failed" | "declined")
    )
}

/// Keep native commentary separate from final assistant text.
pub(super) fn agent_role_from_phase(item: &serde_json::Map<String, Value>) -> Role {
    match item.get("phase").and_then(Value::as_str) {
        Some("commentary") => Role::Commentary,
        _ => Role::Assistant,
    }
}

// --- Reasoning summary ---

/// Join nonempty native reasoning fragments without terminal markup.
pub(crate) fn reasoning_summary_text(item: &Value) -> Option<String> {
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
