//! Attach plain tool labels and failure facts while preserving the received body.

use serde_json::Value;

use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role, ToolOutputDetails};
use super::state::SessionState;
use super::tools::{command_execution_diff_output, first_i64_at_paths, strip_terminal_controls};

impl SessionState {
    /// Attach a short label without replacing the tool's original output.
    pub fn describe_tool_output(
        &mut self,
        idx: usize,
        item: &Value,
        key: Option<&BackendEntryKey>,
    ) {
        let Some(message) = self.messages().get(idx) else {
            return;
        };
        let generated = item["id"].as_str().and_then(|_| {
            self.messages()
                .entry_id_for_item(key?)
                .and_then(|id| self.summaries.label(id))
        });
        let is_generated = generated.is_some() && message.kind != MessageKind::Diff;
        let summary = generated
            .or_else(|| self.tool_command_summary(item, key))
            .or_else(|| self.format_tool_item(item, Role::ToolCall))
            .unwrap_or_else(|| "Tool output".to_string());
        let summary = if message.kind == MessageKind::Diff {
            crate::session::tool_diff_summary::diff_summary(message)
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
            generated: is_generated,
            summary,
            failed: tool_failed(item),
        };
        self.transcript_set_tool(idx, details);
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
