//! Read tool labels and source paths from the session before the view lays out rows.

#[cfg(test)]
use carlos_core::session::models::Role;
#[cfg(test)]
use serde_json::Value;

#[cfg(test)]
use super::state::AppState;

#[cfg(test)]
impl AppState {
    pub(super) fn format_tool_item(&self, item: &Value, role: Role) -> Option<String> {
        self.session.format_tool_item(item, role)
    }

    pub(super) fn tool_command_summary(&self, item: &Value) -> Option<String> {
        let key = item["id"].as_str().map(|id| self.item_key(id));
        self.session.tool_command_summary(item, key.as_ref())
    }
}
