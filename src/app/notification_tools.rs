//! Keep completed tool contents and their labels for full and compact views.

use serde_json::Value;

use super::tool_output_items::{command_output_missing, with_streamed_output};
use super::tools::{
    command_execution_diff_output, command_summary_from_shell_cmd, extract_diff_blocks,
    first_i64_at_paths, format_tool_item, role_for_tool_type, tool_command,
};
use super::transcript_source::source_path_from_item;
use super::{AppState, DiffBlock, MessageKind, Role};

/// Save a completed tool result without dropping its contents.
pub(super) fn complete_tool_item(app: &mut AppState, kind: &str, item_value: &Value) -> bool {
    let Some(mut role) = role_for_tool_type(kind) else {
        return true;
    };
    if kind == "commandExecution" {
        role = Role::ToolOutput;
    }
    if command_output_missing(item_value) {
        if let Some(id) = item_value
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| app.is_item_completed(id))
        {
            if let Some(&idx) = app.item_to_message_index.get(id) {
                app.describe_tool_output(idx, item_value);
            }
            return true;
        }
    }
    let item_value = with_streamed_output(app, item_value);
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
    if diffs.is_empty() {
        return complete_tool_item_no_diff(app, item, role, &item_value);
    }
    complete_tool_item_with_diffs(app, &item_value, role, diffs)
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
            if role == Role::ToolOutput {
                app.describe_tool_output(app.item_to_message_index[id], item_value);
            }
            return true;
        }
        let idx = app.append_message(role, text);
        app.messages[idx].file_path = source_path_from_item(item_value);
        if role == Role::ToolOutput {
            app.describe_tool_output(idx, item_value);
        }
        return true;
    }
    if let Some(id) = item_id {
        app.upsert_mapped_message(id, role, String::new(), MessageKind::Plain, None);
    }
    true
}

fn complete_tool_item_with_diffs(
    app: &mut AppState,
    item: &Value,
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
            app.describe_tool_output(app.item_to_message_index[&key], item);
        }
        return true;
    }
    for block in diffs {
        let idx = app.append_diff_message(role, block.file_path, block.diff);
        app.describe_tool_output(idx, item);
    }
    true
}
