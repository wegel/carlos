//! Adapt checked daemon input descriptions to the shared native answer editor.

use carlos_core::session::questions::{mcp_fields, QuestionField};
use carlos_proto::session::ApprovalInput;

use super::claude_question_state::NativeDraft;

impl NativeDraft {
    /// Decision-only callbacks use the same empty editor and viewport as the embedded terminal.
    pub(super) fn from_approval(input: Option<&ApprovalInput>) -> Self {
        input.map(Self::from_input).unwrap_or_else(|| {
            Self::from_description(
                carlos_core::session::claude::callback_questions::ClaudeQuestion {
                    fields: Vec::new(),
                    unsupported: None,
                    defaults: false,
                },
            )
        })
    }
    /// Build the same local editor from checked named wire fields or the native form schema.
    pub(super) fn from_input(input: &ApprovalInput) -> Self {
        let (fields, unsupported, defaults) = match input {
            ApprovalInput::Fields { fields } => (
                fields
                    .iter()
                    .cloned()
                    .map(QuestionField::from_input_field)
                    .collect(),
                None,
                false,
            ),
            ApprovalInput::Form { schema } => match mcp_fields(schema) {
                Ok(fields) => (fields, None, true),
                Err(error) => (Vec::new(), Some(error), true),
            },
            ApprovalInput::Browser { .. } => (Vec::new(), None, false),
            ApprovalInput::Unsupported { reason } => (Vec::new(), Some(reason.clone()), false),
        };
        let mut draft = Self::from_description(
            carlos_core::session::claude::callback_questions::ClaudeQuestion {
                fields,
                unsupported,
                defaults,
            },
        );
        if !defaults {
            for field in &mut draft.fields {
                field.omitted = false;
            }
        }
        draft
    }
}
