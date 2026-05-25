//! Wrapped-line counters for transcript messages, used to size the scrollback height.

use super::{
    contains_terminal_escapes, count_ascii_multiline_by_cells,
    count_ascii_multiline_by_cells_cached, fast_reasoning_summary_plain_text, RenderCountCache,
};
use crate::app::models::{Role, StyledSegment};
use crate::app::text::wrap_natural_count_by_cells;
use crate::app::tools::strip_terminal_controls;
use crate::app::transcript_styles::{
    ansi_line_segments, is_fence_delimiter, markdown_line_segments, styled_plain_text,
};

pub(in crate::app) fn count_wrapped_message_lines(role: Role, text: &str, width: usize) -> usize {
    if width < 8 {
        return 0;
    }
    if !matches!(role, Role::User) {
        if let Some(count) = count_ascii_multiline_by_cells(text, width, true) {
            return count;
        }
    }

    let mut count = 0usize;
    let mut in_code_fence = false;

    for logical in text.split('\n') {
        if is_fence_delimiter(logical) {
            if matches!(role, Role::User) {
                count += 1;
            }
            in_code_fence = !in_code_fence;
            continue;
        }

        if logical.is_empty() {
            count += 1;
            continue;
        }

        let _ = in_code_fence;
        count += wrap_natural_count_by_cells(logical, width);
    }

    count
}

pub(in crate::app) fn count_wrapped_message_lines_cached<'a>(
    cache: &mut RenderCountCache<'a>,
    role: Role,
    text: &'a str,
    width: usize,
) -> usize {
    if width < 8 {
        return 0;
    }
    if !matches!(role, Role::User) {
        if let Some(count) = count_ascii_multiline_by_cells_cached(cache, text, width, true) {
            return count;
        }
    }
    count_wrapped_message_lines(role, text, width)
}

pub(in crate::app) fn count_wrapped_markdown_lines(role: Role, text: &str, width: usize) -> usize {
    if width < 8 {
        return 0;
    }

    let logical_lines = if matches!(role, Role::Reasoning) {
        let mut count = 0usize;
        let mut used_fast_path = false;
        for raw_line in text.split('\n') {
            if raw_line.is_empty() {
                continue;
            }
            if let Some(plain) = fast_reasoning_summary_plain_text(raw_line) {
                count += wrap_natural_count_by_cells(plain, width);
                used_fast_path = true;
                continue;
            }
            let lines = markdown_line_segments(raw_line);
            if lines.is_empty() {
                continue;
            }
            used_fast_path = true;
            for logical in lines {
                let plain = styled_plain_text(&logical);
                if plain.is_empty() {
                    count += 1;
                } else {
                    count += wrap_natural_count_by_cells(&plain, width);
                }
            }
        }
        if used_fast_path {
            return count;
        } else {
            markdown_line_segments(text)
        }
    } else {
        markdown_line_segments(text)
    };

    logical_lines
        .into_iter()
        .map(|logical| {
            let plain = styled_plain_text(&logical);
            if plain.is_empty() {
                1
            } else {
                wrap_natural_count_by_cells(&plain, width)
            }
        })
        .sum()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::app) fn count_wrapped_ansi_lines(role: Role, text: &str, width: usize) -> usize {
    if width < 8 {
        return 0;
    }
    if !contains_terminal_escapes(text) {
        return count_wrapped_message_lines(role, text, width);
    }
    let Some(logical_lines) = ansi_line_segments(text) else {
        let plain = strip_terminal_controls(text);
        return count_wrapped_message_lines(role, &plain, width);
    };
    count_styled_logical_lines(logical_lines, width)
}

pub(in crate::app) fn count_wrapped_ansi_lines_cached<'a>(
    cache: &mut RenderCountCache<'a>,
    role: Role,
    text: &'a str,
    width: usize,
) -> usize {
    if width < 8 {
        return 0;
    }
    if !contains_terminal_escapes(text) {
        return count_wrapped_message_lines_cached(cache, role, text, width);
    }
    let Some(logical_lines) = ansi_line_segments(text) else {
        let plain = strip_terminal_controls(text);
        return count_wrapped_message_lines(role, &plain, width);
    };
    count_styled_logical_lines(logical_lines, width)
}

pub(in crate::app) fn count_styled_logical_lines(
    logical_lines: Vec<Vec<StyledSegment>>,
    width: usize,
) -> usize {
    logical_lines
        .into_iter()
        .map(|logical| {
            let plain = styled_plain_text(&logical);
            if plain.is_empty() {
                1
            } else {
                wrap_natural_count_by_cells(&plain, width)
            }
        })
        .sum()
}
