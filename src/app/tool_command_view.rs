//! Read tool labels and source paths from the session before the view lays out rows.

use serde_json::Value;

use super::models::Role;
use super::state::AppState;
#[cfg(test)]
pub(super) use crate::session::tool_command_view::ToolCommandView;

impl AppState {
    pub(super) fn format_tool_item(&self, item: &Value, role: Role) -> Option<String> {
        self.session.format_tool_item(item, role)
    }

    pub(super) fn tool_command_summary(&self, item: &Value) -> Option<String> {
        let key = item["id"].as_str().map(|id| self.item_key(id));
        self.session.tool_command_summary(item, key.as_ref())
    }

    pub(super) fn tool_source_path(&self, item: &Value) -> Option<String> {
        self.session.tool_source_path(item)
    }
}
