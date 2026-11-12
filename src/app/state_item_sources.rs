//! Pass the current backend key into terminal-free source matching.

use crate::app::state::AppState;

impl AppState {
    pub(in crate::app) fn prepare_item_source(
        &mut self,
        item: &str,
        source: Option<&serde_json::Value>,
        all_parts: bool,
    ) {
        let key = self.item_key(item);
        self.session
            .prepare_item_source(&key, source, all_parts, self.claude.enabled);
    }

    pub(in crate::app) fn attach_item_source(
        &mut self,
        item: &str,
        source: Option<&serde_json::Value>,
        all_parts: bool,
    ) {
        let key = self.item_key(item);
        self.session.attach_item_source(&key, source, all_parts);
    }
}
