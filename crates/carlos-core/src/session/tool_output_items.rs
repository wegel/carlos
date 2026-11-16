//! Attach plain tool labels and failure facts while preserving the received body.

use carlos_proto::session::ToolStatus;
use serde_json::Value;

use super::entry_key::BackendEntryKey;
use super::models::{MessageKind, Role, ToolOutputDetails};
use super::state::SessionState;
use super::tools::strip_terminal_controls;

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
        let facts = super::tool_facts::collect(message, item);
        let details = ToolOutputDetails {
            generated: is_generated,
            summary,
            failed: matches!(facts.status, ToolStatus::Failed | ToolStatus::Interrupted),
        };
        self.messages.set_tool_facts(idx, facts);
        self.transcript_set_tool(idx, details);
    }

    pub(super) fn describe_tool_facts(&mut self, index: usize, item: &Value) {
        let Some(message) = self.messages().get(index) else {
            return;
        };
        let facts = super::tool_facts::collect(message, item);
        self.messages.set_tool_facts(index, facts);
    }
}
