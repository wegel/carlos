//! Read tool output and invalidate the corresponding cached rows after a session edit.

use std::borrow::Cow;

use serde_json::Value;

use super::models::Role;
use super::state::AppState;

impl AppState {
    pub(super) fn describe_tool_output(&mut self, idx: usize, item: &Value) {
        let key = item["id"].as_str().map(|id| self.item_key(id));
        let update = self.session.describe_tool_output(idx, item, key.as_ref());
        self.apply_session_update(update);
    }
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
            .and_then(|id| app.item_message_index(id))
            .and_then(|idx| app.session.messages().get(idx));
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
