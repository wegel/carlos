//! Builds independent native answer drafts from SDK question payloads.

use std::cell::Cell;

use super::claude_question_field::AnswerField;

/// One callback owns all answer fields until the SDK accepts or cancels it.
pub(super) struct NativeDraft {
    pub(super) fields: Vec<AnswerField>,
    pub(super) current: usize,
    pub(super) review: bool,
    pub(super) viewport: QuestionViewport,
    pub(super) error: Option<String>,
    pub(super) unsupported: Option<String>,
    pub(super) notice: Option<String>,
}

/// Keep the focused option visible until the user explicitly scrolls the question.
#[derive(Default)]
pub(super) struct QuestionViewport {
    pub(super) scroll: Cell<usize>,
    pub(super) max_scroll: Cell<usize>,
    pub(super) follow_focus: Cell<bool>,
}

impl NativeDraft {
    /// Create local editors and apply offered defaults without retaining a second description.
    pub(super) fn from_description(
        description: carlos_core::session::claude::callback_questions::ClaudeQuestion,
    ) -> Self {
        let fields = description
            .fields
            .into_iter()
            .map(|spec| {
                let mut field = AnswerField::from_description(spec);
                if description.defaults {
                    field.apply_default();
                }
                field
            })
            .collect();
        let draft = Self {
            fields,
            current: 0,
            review: false,
            viewport: QuestionViewport::default(),
            error: None,
            unsupported: description.unsupported,
            notice: None,
        };
        draft.viewport.follow_focus.set(true);
        draft
    }

    /// Move between questions and a review page without discarding any drafts.
    pub(super) fn advance(&mut self, backwards: bool) {
        let count = self.fields.len() + 1;
        let index = if self.review {
            self.fields.len()
        } else {
            self.current
        };
        let index = if backwards {
            (index + count - 1) % count
        } else {
            (index + 1) % count
        };
        self.review = index == self.fields.len();
        self.current = index.min(self.fields.len().saturating_sub(1));
        self.viewport.scroll.set(0);
        self.viewport.follow_focus.set(true);
    }
}

/// Keep editing disabled when the terminal can only show the resize/cancel fallback.
pub(super) fn can_edit_question(size: super::TerminalSize) -> bool {
    size.width >= 32 && size.height >= 12
}

#[cfg(test)]
#[path = "../tests/claude_question_parser_tests.rs"]
mod parser_tests;
