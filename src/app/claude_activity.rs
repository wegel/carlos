//! Format native process activity for terminal status and task panels.

use crate::session::claude::{ClaudeActivity, NativeState};

/// Render a short native activity label independently of the main prompt's completion.
pub(super) fn label(activity: &ClaudeActivity) -> Option<String> {
    if let Some(transient) = activity.transient() {
        return Some(transient.to_string());
    }
    if let Some(tokens) = activity.thinking() {
        return Some(format!("Thinking ~{tokens} tokens"));
    }
    match activity.native_state() {
        NativeState::Idle => None,
        NativeState::Running => Some("Running".to_string()),
        NativeState::RequiresAction => Some("Requires action".to_string()),
    }
}
