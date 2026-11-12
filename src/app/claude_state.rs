//! Retain callback answer editors until milestone 7 separates their facts and widgets.

use super::claude_question_state::NativeAnswers;

pub(super) use crate::session::claude::ClaudePhase;

/// Keep only the callback and question state that milestone 7 will split.
#[derive(Default)]
pub(super) struct ClaudeUiState {
    pub(super) answers: NativeAnswers,
}
