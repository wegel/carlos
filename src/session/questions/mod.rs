//! Parse native question descriptions independently of answer editors.

mod claude;
mod codex;
mod field;

pub(crate) use claude::{ask_fields, mcp_fields};
pub(crate) use codex::question_fields;
pub(crate) use field::QuestionField;

#[cfg(test)]
mod claude_tests;
