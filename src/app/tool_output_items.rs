//! Read tool output and invalidate the corresponding cached rows after a session edit.

#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
use super::state::AppState;

#[cfg(test)]
impl AppState {
    pub(super) fn describe_tool_output(&mut self, idx: usize, item: &Value) {
        let key = item["id"].as_str().map(|id| self.item_key(id));
        self.session.describe_tool_output(idx, item, key.as_ref());
        self.consume_core_events();
    }
}
