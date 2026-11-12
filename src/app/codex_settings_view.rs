//! Retain rejected settings edits and the next unsent save preference locally.

use serde_json::Value;

/// Keep settings editor errors and attempted values on this terminal.
#[derive(Default)]
pub(super) struct SettingsView {
    pub(super) editor_error: Option<String>,
    pub(super) rejected: Option<Value>,
    pub(super) persist_next: bool,
}
