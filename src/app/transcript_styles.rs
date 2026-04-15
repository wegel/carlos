//! Core styled-text types, segment manipulation, ANSI conversion, and markdown parsing.
//! Wrapping/counting logic lives in `transcript_wrap`; style conversion in `style_convert`.

use std::collections::VecDeque;

use ansi_to_tui::IntoText as _;

use super::models::{StyledSegment};
use super::style_convert::{core_style_to_style, CarlosMarkdownStyleSheet};
use super::text::{split_at_cells, visual_width};
use tui_markdown::{
    from_str_with_options as markdown_from_str_with_options, Options as MarkdownOptions,
};

// Re-export everything from transcript_wrap so callers of transcript_styles keep working.
pub(super) use super::transcript_wrap::{
    append_wrapped_ansi_lines, append_wrapped_markdown_lines, append_wrapped_message_lines,
    append_wrapped_styled_logical_lines, count_styled_logical_lines, count_wrapped_ansi_lines,
    count_wrapped_ansi_lines_cached, count_wrapped_markdown_lines, count_wrapped_message_lines,
    count_wrapped_message_lines_cached, RenderCountCache,
};

// --- Segment Building ---

pub(super) fn is_fence_delimiter(line: &str) -> bool {
    line.trim_matches([' ', '\t', '\r']).starts_with("```")
}

pub(super) fn styled_plain_text(segments: &[StyledSegment]) -> String {
    let mut out = String::new();
    for seg in segments {
        out.push_str(&seg.text);
    }
    out
}

pub(super) fn markdown_line_segments(text: &str) -> Vec<Vec<StyledSegment>> {
    let opts = MarkdownOptions::new(CarlosMarkdownStyleSheet);
    let markdown = markdown_from_str_with_options(text, &opts);

    let mut out = Vec::new();
    for line in markdown.lines {
        let mut segments = Vec::new();
        for span in line.spans {
            if span.content.is_empty() {
                continue;
            }
            segments.push(StyledSegment {
                text: span.content.to_string(),
                style: core_style_to_style(span.style),
            });
        }
        strip_code_line_gutter(&mut segments);

        let plain = styled_plain_text(&segments);
        if is_fence_delimiter(&plain) {
            continue;
        }
        out.push(segments);
    }
    out
}

fn strip_code_line_gutter(segments: &mut Vec<StyledSegment>) {
    let Some(first) = segments.first_mut() else {
        return;
    };
    // Temporary workaround for the pinned tui-markdown fork, which always prefixes
    // fenced code lines with a `NNN │ ` gutter and does not yet expose a way to disable it.
    let Some(prefix_len) = code_line_gutter_prefix_len(&first.text) else {
        return;
    };
    let remainder = first.text[prefix_len..].to_string();
    if remainder.is_empty() {
        segments.remove(0);
    } else {
        first.text = remainder;
    }
}

fn code_line_gutter_prefix_len(text: &str) -> Option<usize> {
    let mut chars = text.char_indices().peekable();

    while let Some((_, ch)) = chars.peek().copied() {
        if ch == ' ' {
            chars.next();
        } else {
            break;
        }
    }

    let mut saw_digit = false;
    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }

    let mut saw_post_digit_space = false;
    while let Some((_, ch)) = chars.peek().copied() {
        if ch == ' ' {
            saw_post_digit_space = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_post_digit_space {
        return None;
    }

    let (_, bar) = chars.next()?;
    if bar != '│' {
        return None;
    }
    let (space_idx, space) = chars.next()?;
    if space != ' ' {
        return None;
    }
    Some(space_idx + space.len_utf8())
}

pub(super) fn ansi_line_segments(text: &str) -> Option<Vec<Vec<StyledSegment>>> {
    let parsed = text.into_text().ok()?;
    let text_style = parsed.style;
    let mut out = Vec::new();

    for line in parsed.lines {
        let line_style = text_style.patch(line.style);
        let mut segments = Vec::new();
        for span in line.spans {
            if span.content.is_empty() {
                continue;
            }
            segments.push(StyledSegment {
                text: span.content.into_owned(),
                style: core_style_to_style(line_style.patch(span.style)),
            });
        }
        out.push(segments);
    }

    Some(out)
}

pub(super) fn take_styled_segments_by_cells(
    remaining: &mut VecDeque<StyledSegment>,
    max_cells: usize,
) -> Vec<StyledSegment> {
    if max_cells == 0 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut taken_cells = 0usize;

    while let Some(mut seg) = remaining.pop_front() {
        let seg_cells = visual_width(&seg.text);
        if seg_cells == 0 {
            out.push(seg);
            continue;
        }

        if taken_cells + seg_cells <= max_cells {
            taken_cells += seg_cells;
            out.push(seg);
            if taken_cells == max_cells {
                break;
            }
            continue;
        }

        let allowed = max_cells.saturating_sub(taken_cells);
        if allowed == 0 {
            remaining.push_front(seg);
            break;
        }

        let split = split_at_cells(&seg.text, allowed);
        if split == 0 {
            remaining.push_front(seg);
            break;
        }

        let left = seg.text[..split].to_string();
        let right = seg.text[split..].to_string();
        let seg_style = seg.style;
        if !right.is_empty() {
            seg.text = right;
            remaining.push_front(seg);
        }

        out.push(StyledSegment {
            text: left,
            style: seg_style,
        });
        break;
    }

    out
}

pub(super) fn normalize_styled_segments_for_part(
    part: &str,
    styled_segments: Vec<StyledSegment>,
) -> Vec<StyledSegment> {
    use ratatui::style::Style;
    if styled_plain_text(&styled_segments) == part {
        styled_segments
    } else {
        vec![StyledSegment {
            text: part.to_string(),
            style: Style::default(),
        }]
    }
}
