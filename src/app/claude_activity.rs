//! Shows native main-process activity without turning background work into human turns.

use serde_json::Value;

/// Keeps native session state, temporary work labels, and context-read freshness together.
#[derive(Default)]
pub(super) struct ClaudeActivity {
    pub(super) state: NativeState,
    pub(super) transient: Option<String>,
    pub(super) thinking: Option<u64>,
    pub(super) context_revision: u64,
    pub(super) compact: Option<Compaction>,
    pub(super) compact_sequence: u64,
}

/// Distinguishes a waiting user callback from model activity and an idle main process.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeState {
    #[default]
    Idle,
    Running,
    RequiresAction,
}

/// Gives one compaction attempt one visible notice, even when both status and boundary arrive.
pub(super) struct Compaction {
    pub(super) key: String,
    pub(super) boundary: Option<String>,
    pub(super) result: Option<String>,
}

impl ClaudeActivity {
    /// Apply native state and invalidate old context counts before fresh work changes them.
    pub(super) fn state(&mut self, message: &Value) {
        self.state = match message["state"].as_str() {
            Some("running") => NativeState::Running,
            Some("requires_action") => NativeState::RequiresAction,
            _ => NativeState::Idle,
        };
        if self.state == NativeState::Running {
            self.context_revision += 1;
        } else {
            self.transient = None;
            self.thinking = None;
        }
    }

    /// Render a short native activity label independently of the main prompt's completion.
    pub(super) fn label(&self) -> Option<String> {
        if let Some(transient) = &self.transient {
            return Some(transient.clone());
        }
        if let Some(tokens) = self.thinking {
            return Some(format!("Thinking ~{tokens} tokens"));
        }
        match self.state {
            NativeState::Idle => None,
            NativeState::Running => Some("Running".to_string()),
            NativeState::RequiresAction => Some("Requires action".to_string()),
        }
    }
}
