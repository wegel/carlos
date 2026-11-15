//! Parse native question descriptions independently of answer editors.

mod caller;
mod claude;
mod codex;
mod field;

pub use caller::caller_label;
pub(crate) use claude::ask_fields;
pub use claude::mcp_fields;
pub(crate) use codex::question_fields;
pub use field::QuestionField;

#[cfg(test)]
mod claude_tests;
