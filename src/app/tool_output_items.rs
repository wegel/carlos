//! Keep tool contents intact and attach labels and failures for the screen.

use std::borrow::Cow;

use serde_json::Value;

use super::models::{MessageKind, Role, ToolOutputDetails};
use super::state::AppState;
use super::tools::{
    command_execution_diff_output, command_summary_from_shell_cmd, first_i64_at_paths,
    format_tool_item, strip_terminal_controls, tool_command,
};

impl AppState {
    /// Attach a short label without replacing the tool's original output.
    pub(super) fn describe_tool_output(&mut self, idx: usize, item: &Value) {
        let Some(message) = self.messages.get(idx) else {
            return;
        };
        let summary = item
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| self.command_override(id))
            .or_else(|| {
                tool_command(item)
                    .and_then(|command| command_summary_from_shell_cmd(&command, None))
            })
            .or_else(|| format_tool_item(item, Role::ToolCall))
            .unwrap_or_else(|| "Tool output".to_string());
        let summary = if message.kind == MessageKind::Diff {
            super::tool_output_state::diff_summary(message)
        } else {
            summary
        };
        let summary = strip_terminal_controls(&summary)
            .lines()
            .next()
            .unwrap_or("Tool output")
            .chars()
            .collect();
        let details = ToolOutputDetails {
            summary,
            failed: tool_failed(item),
        };
        self.messages[idx].tool_output = Some(details);
        self.mark_transcript_dirty_from(idx);
    }
}

fn tool_failed(item: &Value) -> bool {
    let exit = first_i64_at_paths(
        item,
        &[
            &["exitCode"],
            &["exit_code"],
            &["metadata", "exitCode"],
            &["metadata", "exit_code"],
            &["state", "exitCode"],
        ],
    )
    .unwrap_or(0);
    let diff_exit = exit == 1 && command_execution_diff_output(item).is_some();
    (exit != 0 && !diff_exit)
        || ["isError", "is_error"]
            .iter()
            .any(|key| item.get(key).and_then(Value::as_bool) == Some(true))
        || item
            .get("status")
            .and_then(Value::as_str)
            .is_some_and(|status| matches!(status, "failed" | "declined"))
}

/// A final command item may carry status without repeating earlier output deltas.
pub(super) fn command_output_missing(item: &Value) -> bool {
    item.get("type").and_then(Value::as_str) == Some("commandExecution")
        && [
            "aggregatedOutput",
            "aggregated_output",
            "output",
            "stdout",
            "stderr",
            "formattedOutput",
            "formatted_output",
        ]
        .iter()
        .all(|key| {
            item.get(key)
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
        })
}

/// Reuse received deltas when the backend omits the body from its final item.
pub(super) fn with_streamed_output<'a>(app: &AppState, item: &'a Value) -> Cow<'a, Value> {
    if command_output_missing(item) {
        let message = item
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| app.item_to_message_index.get(id))
            .and_then(|idx| app.messages.get(*idx));
        if let Some(message) =
            message.filter(|message| message.role == Role::ToolOutput && !message.text.is_empty())
        {
            let mut item = item.clone();
            item["aggregatedOutput"] = Value::String(message.text.clone());
            return Cow::Owned(item);
        }
    }
    Cow::Borrowed(item)
}
