//! Share blank-line rules between the desktop gutter and phone section layout.

#[path = "transcript_one_row.rs"]
mod one_row;

use carlos_core::session::models::Role;

use self::one_row::reasoning_uses_one_row;
use super::transcript_input::RenderInput;

/// Group reasoning with tools only when its text fits one row at this width.
pub(super) fn single_line_reasoning(message: RenderInput<'_>, width: usize) -> bool {
    width >= 8
        && message.role == Role::Reasoning
        && !message.text.trim().contains('\n')
        && reasoning_uses_one_row(message.text, width)
}

/// Keep tools and short reasoning together inside one activity block.
pub(super) fn inline_entry(message: RenderInput<'_>, width: usize) -> bool {
    matches!(message.role, Role::ToolCall | Role::ToolOutput)
        || single_line_reasoning(message, width)
}

#[cfg(test)]
#[path = "transcript_spacing_tests.rs"]
mod tests;
