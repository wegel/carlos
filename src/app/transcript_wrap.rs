//! Wrapping and line-count logic: `append_wrapped_*` and `count_wrapped_*` functions,
//! cell-accurate natural wrapping, and caching helpers for ASCII long-lines.

use std::collections::{HashMap, VecDeque};

use ratatui::style::Style;

use super::models::{RenderedLine, Role, StyledSegment};
use super::text::{visual_width, wrap_natural_by_cells, wrap_natural_count_by_cells};
use super::tools::strip_terminal_controls;
use super::transcript_styles::{
    ansi_line_segments, is_fence_delimiter, markdown_line_segments, normalize_styled_segments_for_part,
    styled_plain_text, take_styled_segments_by_cells,
};
use crate::theme::COLOR_STEP2;

// --- Cache ---

pub(super) struct RenderCountCache<'a> {
    ascii_long_line_counts: HashMap<&'a str, usize>,
}

impl<'a> RenderCountCache<'a> {
    pub(super) fn new() -> Self {
        Self { ascii_long_line_counts: HashMap::new() }
    }
}

// --- Internal helpers ---

fn contains_terminal_escapes(text: &str) -> bool {
    text.contains('\u{1b}') || text.contains('\u{009b}')
}

fn fast_reasoning_summary_plain_text(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("**")?.strip_suffix("**")?;
    if inner.is_empty() || inner.contains("**") {
        return None;
    }
    Some(inner.trim_end_matches(' '))
}

fn count_ascii_multiline_by_cells(
    text: &str,
    width: usize,
    skip_fence_delimiters: bool,
) -> Option<usize> {
    if width == 0 || !text.is_ascii() {
        return None;
    }
    let mut count = 0usize;
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for idx in 0..=bytes.len() {
        if idx != bytes.len() && bytes[idx] != b'\n' {
            continue;
        }
        let line = &text[start..idx];
        count += if skip_fence_delimiters && line.trim_matches([' ', '\t', '\r']).starts_with("```") {
            0
        } else if line.is_empty() || line.len() <= width {
            1
        } else {
            wrap_natural_count_by_cells(line, width)
        };
        start = idx.saturating_add(1);
    }
    Some(count)
}

fn count_ascii_multiline_by_cells_cached<'a>(
    cache: &mut RenderCountCache<'a>,
    text: &'a str,
    width: usize,
    skip_fence_delimiters: bool,
) -> Option<usize> {
    if width == 0 || !text.is_ascii() {
        return None;
    }
    let mut count = 0usize;
    let mut start = 0usize;
    let bytes = text.as_bytes();
    for idx in 0..=bytes.len() {
        if idx != bytes.len() && bytes[idx] != b'\n' {
            continue;
        }
        let line = &text[start..idx];
        count += if skip_fence_delimiters && line.trim_matches([' ', '\t', '\r']).starts_with("```") {
            0
        } else if line.is_empty() || line.len() <= width {
            1
        } else if let Some(cached) = cache.ascii_long_line_counts.get(line) {
            *cached
        } else {
            let computed = wrap_natural_count_by_cells(line, width);
            cache.ascii_long_line_counts.insert(line, computed);
            computed
        };
        start = idx.saturating_add(1);
    }
    Some(count)
}

// --- RenderedLine constructors ---

#[inline]
fn blank_line(role: Role) -> RenderedLine {
    RenderedLine { cells: 0, text: String::new(), styled_segments: Vec::new(), role, separator: false, soft_wrap_to_next: false }
}

#[inline]
fn content_line(role: Role, part: &str, cells: usize, segments: Vec<StyledSegment>, wrapped: bool) -> RenderedLine {
    RenderedLine { cells, text: part.to_owned(), styled_segments: segments, role, separator: false, soft_wrap_to_next: wrapped }
}

// --- Shared inner loop ---

/// Wrap a list of styled logical lines and push the resulting `RenderedLine`s into `out`.
/// This is the common body shared by markdown, ANSI, and diff append functions.
fn append_styled_logical_lines_inner(
    out: &mut Vec<RenderedLine>,
    role: Role,
    logical_lines: Vec<Vec<StyledSegment>>,
    width: usize,
) {
    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            out.push(blank_line(role));
            continue;
        }
        let wrapped_parts = wrap_natural_by_cells(&plain, width);
        let mut remaining: VecDeque<StyledSegment> = logical.into();
        for (i, part) in wrapped_parts.iter().enumerate() {
            let part_cells = visual_width(part);
            let segs = normalize_styled_segments_for_part(
                part,
                take_styled_segments_by_cells(&mut remaining, part_cells),
            );
            out.push(content_line(role, part, part_cells, segs, i + 1 < wrapped_parts.len()));
        }
    }
}

// --- Append ---

pub(super) fn append_wrapped_message_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    text: &str,
    width: usize,
) {
    if width < 8 {
        return;
    }

    let mut in_code_fence = false;

    for logical in text.split('\n') {
        if is_fence_delimiter(logical) {
            if matches!(role, Role::User) {
                out.push(blank_line(role));
            }
            in_code_fence = !in_code_fence;
            continue;
        }
        if logical.is_empty() {
            out.push(blank_line(role));
            continue;
        }
        let wrapped_parts = wrap_natural_by_cells(logical, width);
        for (i, part) in wrapped_parts.iter().enumerate() {
            let fence_style = if matches!(role, Role::User) && in_code_fence {
                Style::default().bg(COLOR_STEP2)
            } else {
                Style::default()
            };
            let segs = vec![StyledSegment { text: part.clone(), style: fence_style }];
            out.push(content_line(role, part, visual_width(part), segs, i + 1 < wrapped_parts.len()));
        }
    }
}

pub(super) fn append_wrapped_markdown_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    text: &str,
    width: usize,
) {
    if width < 8 {
        return;
    }

    let logical_lines = if matches!(role, Role::Reasoning) {
        let mut lines = Vec::new();
        for raw_line in text.split('\n') {
            if raw_line.is_empty() {
                continue;
            }
            lines.extend(markdown_line_segments(raw_line));
        }
        if lines.is_empty() {
            markdown_line_segments(text)
        } else {
            lines
        }
    } else {
        markdown_line_segments(text)
    };
    append_styled_logical_lines_inner(out, role, logical_lines, width);
}

pub(super) fn append_wrapped_styled_logical_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    logical_lines: Vec<Vec<StyledSegment>>,
    width: usize,
) {
    if width < 8 {
        return;
    }
    append_styled_logical_lines_inner(out, role, logical_lines, width);
}

pub(super) fn append_wrapped_ansi_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    text: &str,
    width: usize,
) {
    if width < 8 {
        return;
    }

    if !contains_terminal_escapes(text) {
        append_wrapped_message_lines(out, role, text, width);
        return;
    }

    let Some(logical_lines) = ansi_line_segments(text) else {
        append_wrapped_message_lines(out, role, text, width);
        return;
    };

    append_styled_logical_lines_inner(out, role, logical_lines, width);
}

// --- Counting ---

pub(super) fn count_wrapped_message_lines(role: Role, text: &str, width: usize) -> usize {
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

pub(super) fn count_wrapped_message_lines_cached<'a>(
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

pub(super) fn count_wrapped_markdown_lines(role: Role, text: &str, width: usize) -> usize {
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

    logical_lines.into_iter().map(|logical| {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() { 1 } else { wrap_natural_count_by_cells(&plain, width) }
    }).sum()
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn count_wrapped_ansi_lines(role: Role, text: &str, width: usize) -> usize {
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

pub(super) fn count_wrapped_ansi_lines_cached<'a>(
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

pub(super) fn count_styled_logical_lines(logical_lines: Vec<Vec<StyledSegment>>, width: usize) -> usize {
    logical_lines.into_iter().map(|logical| {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() { 1 } else { wrap_natural_count_by_cells(&plain, width) }
    }).sum()
}
