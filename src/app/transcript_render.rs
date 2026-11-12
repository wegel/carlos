//! Shared transcript entry points and read-label coalescing helpers.

#[cfg(test)]
use super::layout_state::TranscriptTheme;
#[cfg(test)]
use super::models::{Message, Role};
#[cfg(test)]
use super::transcript_block::{has_visible_content, BlockPlan};
#[cfg(test)]
pub(super) use super::transcript_styles::normalize_styled_segments_for_part;
pub(super) use super::transcript_styles::RenderCountCache;
#[cfg(test)]
use super::view_models::{DisplayMessage, RenderedLine};

// --- Transcript Build ---

#[cfg(test)]
pub(super) fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    build_rendered_lines_with_hidden(messages, width, None)
}

#[cfg(test)]
pub(super) fn build_rendered_lines_with_hidden(
    messages: &[Message],
    width: usize,
    hidden_user_message_idx: Option<usize>,
) -> Vec<RenderedLine> {
    let mut out = Vec::new();
    let mut previous_visible: Option<&Message> = None;

    for (idx, msg) in messages.iter().enumerate() {
        if hidden_user_message_idx == Some(idx) && msg.role == Role::User {
            continue;
        }
        if !has_visible_content(msg) {
            continue;
        }
        append_rendered_block_for_message(&mut out, previous_visible, msg, width);
        previous_visible = Some(msg);
    }

    out
}

#[cfg(test)]
pub(super) fn build_rendered_block_for_message(
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) -> Vec<RenderedLine> {
    BlockPlan::new(
        previous_visible,
        DisplayMessage::full(msg),
        width,
        TranscriptTheme::Gutter,
    )
    .render(msg)
}

/// Count a Gutter block through the shared plan.
#[cfg_attr(not(test), allow(dead_code))]
#[cfg(test)]
pub(super) fn count_rendered_block_for_message(
    previous: Option<&Message>,
    message: &Message,
    width: usize,
) -> usize {
    count_rendered_block_for_message_cached(&mut RenderCountCache::new(), previous, message, width)
}

/// Count the same format, spacing, and labels that the renderer uses.
#[cfg(test)]
pub(super) fn count_rendered_block_for_message_cached<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<&Message>,
    message: &'a Message,
    width: usize,
) -> usize {
    BlockPlan::new(
        previous,
        DisplayMessage::full(message),
        width,
        TranscriptTheme::Gutter,
    )
    .count(cache, message)
}

/// Append the rows described by a Gutter block plan.
#[cfg(test)]
pub(super) fn append_rendered_block_for_message(
    out: &mut Vec<RenderedLine>,
    previous: Option<&Message>,
    message: &Message,
    width: usize,
) {
    out.extend(
        BlockPlan::new(
            previous,
            DisplayMessage::full(message),
            width,
            TranscriptTheme::Gutter,
        )
        .render(message),
    );
}
