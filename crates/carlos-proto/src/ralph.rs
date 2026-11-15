//! Capture explicit host prompt-file and stop-marker choices for one Ralph enable request.

use serde::{Deserialize, Serialize};

/// Resolve an optional prompt path against the daemon session's cwd when enabling Ralph.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RalphOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_marker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_marker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_marker: Option<String>,
}

impl RalphOptions {
    /// Distinguish supplied overrides from a request that resets all four choices to defaults.
    pub fn has_overrides(&self) -> bool {
        self.prompt_path.is_some()
            || self.done_marker.is_some()
            || self.blocked_marker.is_some()
            || self.waiting_marker.is_some()
    }
}

#[cfg(test)]
#[path = "ralph_tests.rs"]
mod tests;
