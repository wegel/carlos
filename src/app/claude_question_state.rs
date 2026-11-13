//! Builds independent native answer drafts from SDK question payloads.

use std::cell::Cell;
use std::collections::HashMap;

use serde_json::{json, Map, Value};

use super::claude_question_field::AnswerField;

/// Unanswered callbacks and consented browser requests retain separate lifetimes.
#[derive(Default)]
pub(super) struct NativeAnswers {
    pub(super) drafts: HashMap<String, NativeDraft>,
}

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
    /// Build answer fields without changing the original SDK callback input.
    pub(super) fn from_payload(kind: &str, payload: &Value) -> Self {
        Self::from_description(
            carlos_core::session::claude::callback_questions::ClaudeQuestion::from_payload(
                kind, payload,
            ),
        )
    }

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

    /// Build native answers, preserving question text and every unrelated input field.
    pub(super) fn answer(&self, payload: &Value) -> Result<Value, String> {
        if let Some(error) = &self.unsupported {
            return Err(format!("Unsupported form: {error}; decline or cancel"));
        }
        if payload["toolName"] == "AskUserQuestion" {
            let mut input = payload["input"].clone();
            let mut answers = Map::new();
            for field in &self.fields {
                answers.insert(field.spec.name().to_string(), json!(field.ask_value()?));
            }
            input["answers"] = Value::Object(answers);
            return Ok(json!({ "behavior": "allow", "updatedInput": input }));
        }
        if payload.get("toolName").is_some() {
            return Ok(json!({ "behavior": "allow" }));
        }
        if payload["mode"] == "url" {
            return Ok(json!({ "action": "accept" }));
        }
        let mut content = Map::new();
        for field in &self.fields {
            if let Some(value) = field.mcp_value()? {
                content.insert(field.spec.name().to_string(), value);
            }
        }
        Ok(json!({ "action": "accept", "content": content }))
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
