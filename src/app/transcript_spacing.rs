//! Share blank-line rules between the desktop gutter and phone section layout.

use super::models::{Message, Role};
use super::transcript_styles::count_wrapped_markdown_lines;

/// Group reasoning with tools only when its text fits one row at this width.
pub(super) fn single_line_reasoning(message: &Message, width: usize) -> bool {
    width >= 8
        && message.role == Role::Reasoning
        && !message.text.trim().contains('\n')
        && count_wrapped_markdown_lines(Role::Reasoning, &message.text, width) == 1
}

/// Keep tools and short reasoning together inside one activity block.
pub(super) fn inline_entry(message: &Message, width: usize) -> bool {
    matches!(message.role, Role::ToolCall | Role::ToolOutput)
        || single_line_reasoning(message, width)
}

/// Separate messages with a blank row unless both belong to the activity block.
pub(super) fn should_insert_separator_between(
    previous: &Message,
    next: &Message,
    width: usize,
) -> bool {
    !(inline_entry(previous, width) && inline_entry(next, width))
}
