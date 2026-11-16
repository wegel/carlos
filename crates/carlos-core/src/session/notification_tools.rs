//! Keep completed tool contents and their labels for full and compact views.

use serde_json::Value;

use super::item_receive::{command_output_missing, with_streamed_output};
use super::models::{DiffBlock, MessageKind, Role};
use super::state::SessionState;
use super::tools::{
    command_execution_diff_output, extract_diff_blocks, first_i64_at_paths, native_tool_source,
    role_for_tool_item,
};

/// Save a completed tool result without dropping its contents.
pub(super) fn complete_tool_item(
    session: &mut SessionState,
    kind: &str,
    item_value: &Value,
) -> bool {
    let Some(mut role) = role_for_tool_item(item_value) else {
        return true;
    };
    if kind == "commandExecution" {
        role = Role::ToolOutput;
    }
    if command_output_missing(item_value) {
        if let Some(id) = item_value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| session.is_item_completed(&session.backend_entry_key(id)))
        {
            if let Some(idx) = session.item_message_index(&session.backend_entry_key(id)) {
                super::item_receive::describe_output(session, idx, item_value);
            }
            return true;
        }
    }
    let item_value = with_streamed_output(session, item_value);
    let Some(item) = item_value.as_object() else {
        return true;
    };

    let mut diffs = extract_diff_blocks(&item_value);
    if diffs.is_empty() {
        if let Some(diff) = command_execution_diff_output(&item_value) {
            diffs.push(DiffBlock {
                file_path: None,
                diff,
            });
        }
    }
    if let Some(id) = item.get("id").and_then(Value::as_str) {
        session.transcript_retain_parts(&session.backend_entry_key(id), diffs.len().max(1));
    }
    if diffs.is_empty() {
        return complete_tool_item_no_diff(session, item, role, &item_value);
    }
    complete_tool_item_with_diffs(session, &item_value, role, diffs)
}

fn complete_tool_item_no_diff(
    session: &mut SessionState,
    item: &serde_json::Map<String, Value>,
    role: Role,
    item_value: &Value,
) -> bool {
    let item_id = item.get("id").and_then(Value::as_str);
    let exit_code = first_i64_at_paths(item_value, &[&["exitCode"], &["exit_code"]]);
    let command_summary = session.tool_command_summary(
        item_value,
        item_value["id"]
            .as_str()
            .map(|id| session.backend_entry_key(id))
            .as_ref(),
    );

    if let Some(formatted) =
        native_tool_source(item_value).or_else(|| session.format_tool_item(item_value, role))
    {
        let text = if exit_code.unwrap_or(0) != 0 {
            if let Some(summary) = command_summary {
                format!("{summary}\n{formatted}")
            } else {
                formatted
            }
        } else {
            formatted
        };
        store_tool_body(session, item_id, role, text, item_value);
        return true;
    }
    if let Some(id) = item_id {
        session.upsert_mapped_message(
            &session.backend_entry_key(id),
            role,
            String::new(),
            MessageKind::Plain,
            None,
        );
    }
    true
}

fn complete_tool_item_with_diffs(
    session: &mut SessionState,
    item: &Value,
    role: Role,
    diffs: Vec<DiffBlock>,
) -> bool {
    let item_id = item.get("id").and_then(Value::as_str);
    if let Some(id) = item_id {
        for (index, block) in diffs.into_iter().enumerate() {
            session.with_native_item_part(id, index, |session, key| {
                session.upsert_mapped_message(
                    &session.backend_entry_key(key),
                    role,
                    block.diff,
                    MessageKind::Diff,
                    block.file_path,
                );
                super::item_receive::describe_output(
                    session,
                    session
                        .item_message_index(&session.backend_entry_key(key))
                        .expect("native item row"),
                    item,
                );
            });
        }
        return true;
    }
    for block in diffs {
        let idx = session.append_diff_message(role, block.file_path, block.diff);
        super::item_receive::describe_output(session, idx, item);
    }
    true
}

fn store_tool_body(
    session: &mut SessionState,
    item_id: Option<&str>,
    role: Role,
    text: String,
    item: &Value,
) {
    let index = if let Some(id) = item_id {
        let key = session.backend_entry_key(id);
        session.upsert_mapped_message(
            &key,
            role,
            text,
            MessageKind::Plain,
            session.tool_source_path(item),
        );
        session.item_message_index(&key).expect("native item row")
    } else {
        let index = session.append_message(role, text);
        session.transcript_set_path(index, session.tool_source_path(item));
        index
    };
    if role == Role::ToolOutput || item["type"] == "mcpToolCall" {
        super::item_receive::describe_output(session, index, item);
    } else {
        session.describe_tool_facts(index, item);
    }
}
