//! Apply native item redraw and prompt-cursor reports to this terminal.

use serde_json::{Map, Value};

use super::state::AppState;

/// Pass one common item notification to its session owner.
pub(super) fn handle_item_notification(
    app: &mut AppState,
    method: &str,
    params: &Map<String, Value>,
) -> bool {
    let mut changes = Default::default();
    let handled = crate::session::notification_items::handle_item_notification(
        &mut app.session,
        &mut changes,
        method,
        params,
    );
    app.apply_item_changes(changes);
    handled
}

#[cfg(test)]
#[path = "../tests/idless_item_tests.rs"]
mod idless_item_tests;
