//! Tracks Claude prompt UUIDs across native result cycles and session-state events.

use std::collections::VecDeque;

use super::claude_question_state::NativeAnswers;

pub(super) use crate::session::claude::ClaudePhase;

/// Tracks native Claude messages and unanswered questions for one UI session.
pub(super) struct ClaudeUiState {
    pub(super) enabled: bool,
    pub(super) epoch: u64,
    pub(super) notices: VecDeque<String>,
    pub(super) reset_key: Option<String>,
    pub(super) awaiting_reset_init: bool,
    pub(super) tasks: super::claude_tasks::ClaudeTasks,
    pub(super) activity: super::claude_activity::ClaudeActivity,
    pub(super) prompts: super::claude_prompts::ClaudePrompts,
    pub(super) generation: u64,
    pub(super) history: super::claude_session_history::ClaudeHistory,
    pub(super) answers: NativeAnswers,
}

impl Default for ClaudeUiState {
    fn default() -> Self {
        Self {
            enabled: false,
            epoch: 0,
            notices: Default::default(),
            reset_key: None,
            awaiting_reset_init: false,
            tasks: Default::default(),
            activity: Default::default(),
            prompts: Default::default(),
            generation: 1,
            history: super::claude_session_history::ClaudeHistory::default(),
            answers: NativeAnswers::default(),
        }
    }
}
