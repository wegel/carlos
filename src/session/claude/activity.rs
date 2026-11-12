//! Shows native main-process activity without turning background work into human turns.

use serde_json::Value;

/// Keeps native session state, temporary work labels, and context-read freshness together.
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ClaudeActivity {
    pub(in crate::session) state: NativeState,
    pub(in crate::session) transient: Option<String>,
    pub(in crate::session) thinking: Option<u64>,
    pub(in crate::session) context_revision: u64,
    pub(in crate::session) compact: Option<Compaction>,
    pub(in crate::session) compact_sequence: u64,
}

/// Distinguishes a waiting user callback from model activity and an idle main process.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeState {
    #[default]
    Idle,
    Running,
    RequiresAction,
}

/// Gives one compaction attempt one visible notice, even when both status and boundary arrive.
#[cfg_attr(test, derive(Debug))]
pub(in crate::session) struct Compaction {
    pub(in crate::session) key: String,
    pub(in crate::session) boundary: Option<String>,
    pub(in crate::session) result: Option<String>,
}

impl ClaudeActivity {
    /// Apply native activity without treating repeated running events as a new context.
    pub(in crate::session) fn state(&mut self, message: &Value) {
        self.state = match message["state"].as_str() {
            Some("running") => NativeState::Running,
            Some("requires_action") => NativeState::RequiresAction,
            _ => NativeState::Idle,
        };
        if self.state != NativeState::Running {
            self.transient = None;
            self.thinking = None;
        }
    }
}

impl ClaudeActivity {
    /// Read the main process state independently of human turns.
    pub(crate) fn native_state(&self) -> NativeState {
        self.state
    }

    /// Read whether native Claude reports main-process work.
    pub(crate) fn is_running(&self) -> bool {
        self.state == NativeState::Running
    }

    /// Borrow a temporary native activity label.
    pub(crate) fn transient(&self) -> Option<&str> {
        self.transient.as_deref()
    }

    /// Read the native thinking estimate.
    pub(crate) fn thinking(&self) -> Option<u64> {
        self.thinking
    }

    /// Read the fence used by optional context requests.
    #[cfg(test)]
    pub(crate) fn context_revision(&self) -> u64 {
        self.context_revision
    }
}
