//! Tracks Claude prompt UUIDs across native result cycles and session-state events.

use std::collections::VecDeque;

use serde_json::Value;

use super::claude_question_state::NativeAnswers;

/// Distinguishes a query still starting from a helper that has closed permanently.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ClaudePhase {
    Starting,
    Ready,
    Closed,
}

/// Tracks native Claude messages and unanswered questions for one UI session.
pub(super) struct ClaudeUiState {
    pub(super) phase: ClaudePhase,
    pub(super) enabled: bool,
    pub(super) epoch: u64,
    pub(super) notices: VecDeque<String>,
    pub(super) quota: super::claude_quota::ClaudeQuota,
    pub(super) reset_key: Option<String>,
    pub(super) awaiting_reset_init: bool,
    pub(super) tasks: super::claude_tasks::ClaudeTasks,
    pub(super) activity: super::claude_activity::ClaudeActivity,
    pub(super) context: super::claude_context::ClaudeContext,
    pub(super) prompts: super::claude_prompts::ClaudePrompts,
    pub(super) generation: u64,
    pub(super) history: super::claude_session_history::ClaudeHistory,
    pub(super) capabilities: Option<Value>,
    pub(super) observed_model: Option<String>,
    pub(super) answers: NativeAnswers,
    pub(super) selection: ClaudeSelection,
}

/// Separates native defaults from explicit Carlos model and effort choices.
pub(super) struct ClaudeSelection {
    pub(super) inherits_model: bool,
    pub(super) inherits_effort: bool,
    pub(super) editing: Option<(String, String)>,
}

impl Default for ClaudeSelection {
    fn default() -> Self {
        Self {
            inherits_model: true,
            inherits_effort: true,
            editing: None,
        }
    }
}

impl Default for ClaudeUiState {
    fn default() -> Self {
        Self {
            phase: ClaudePhase::Ready,
            enabled: false,
            epoch: 0,
            notices: Default::default(),
            quota: Default::default(),
            reset_key: None,
            awaiting_reset_init: false,
            tasks: Default::default(),
            activity: Default::default(),
            context: Default::default(),
            prompts: Default::default(),
            generation: 1,
            history: super::claude_session_history::ClaudeHistory::default(),
            capabilities: None,
            observed_model: None,
            answers: NativeAnswers::default(),
            selection: ClaudeSelection::default(),
        }
    }
}
