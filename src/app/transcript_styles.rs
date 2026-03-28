use std::collections::{HashMap, VecDeque};

use ansi_to_tui::IntoText as _;
use ratatui::style::{Color, Modifier, Style};
use ratatui_core::style::{Color as CoreColor, Modifier as CoreModifier, Style as CoreStyle};
use tui_markdown::{
    from_str_with_options as markdown_from_str_with_options, Options as MarkdownOptions,
};

use super::models::{RenderedLine, Role, StyledSegment};
use super::text::{
    split_at_cells, visual_width, wrap_natural_by_cells, wrap_natural_count_by_cells,
};
use super::tools::strip_terminal_controls;
use crate::theme::*;

#[derive(Debug, Clone, Copy)]
struct CarlosMarkdownStyleSheet;

pub(super) struct RenderCountCache<'a> {
    ascii_long_line_counts: HashMap<&'a str, usize>,
}

impl<'a> RenderCountCache<'a> {
    pub(super) fn new() -> Self {
        Self {
            ascii_long_line_counts: HashMap::new(),
        }
    }
}

pub(super) fn color_to_core(color: Color) -> CoreColor {
    match color {
        Color::Reset => CoreColor::Reset,
        Color::Black => CoreColor::Black,
        Color::Red => CoreColor::Red,
        Color::Green => CoreColor::Green,
        Color::Yellow => CoreColor::Yellow,
        Color::Blue => CoreColor::Blue,
        Color::Magenta => CoreColor::Magenta,
        Color::Cyan => CoreColor::Cyan,
        Color::Gray => CoreColor::Gray,
        Color::DarkGray => CoreColor::DarkGray,
        Color::LightRed => CoreColor::LightRed,
        Color::LightGreen => CoreColor::LightGreen,
        Color::LightYellow => CoreColor::LightYellow,
        Color::LightBlue => CoreColor::LightBlue,
        Color::LightMagenta => CoreColor::LightMagenta,
        Color::LightCyan => CoreColor::LightCyan,
        Color::White => CoreColor::White,
        Color::Rgb(r, g, b) => CoreColor::Rgb(r, g, b),
        Color::Indexed(v) => CoreColor::Indexed(v),
    }
}

pub(super) fn core_color_to_color(color: CoreColor) -> Color {
    match color {
        CoreColor::Reset => Color::Reset,
        CoreColor::Black => Color::Black,
        CoreColor::Red => Color::Red,
        CoreColor::Green => Color::Green,
        CoreColor::Yellow => Color::Yellow,
        CoreColor::Blue => Color::Blue,
        CoreColor::Magenta => Color::Magenta,
        CoreColor::Cyan => Color::Cyan,
        CoreColor::Gray => Color::Gray,
        CoreColor::DarkGray => Color::DarkGray,
        CoreColor::LightRed => Color::LightRed,
        CoreColor::LightGreen => Color::LightGreen,
        CoreColor::LightYellow => Color::LightYellow,
        CoreColor::LightBlue => Color::LightBlue,
        CoreColor::LightMagenta => Color::LightMagenta,
        CoreColor::LightCyan => Color::LightCyan,
        CoreColor::White => Color::White,
        CoreColor::Rgb(r, g, b) => Color::Rgb(r, g, b),
        CoreColor::Indexed(v) => Color::Indexed(v),
    }
}

pub(super) fn modifier_to_core(modifier: Modifier) -> CoreModifier {
    let mut out = CoreModifier::empty();
    if modifier.contains(Modifier::BOLD) {
        out |= CoreModifier::BOLD;
    }
    if modifier.contains(Modifier::DIM) {
        out |= CoreModifier::DIM;
    }
    if modifier.contains(Modifier::ITALIC) {
        out |= CoreModifier::ITALIC;
    }
    if modifier.contains(Modifier::UNDERLINED) {
        out |= CoreModifier::UNDERLINED;
    }
    if modifier.contains(Modifier::SLOW_BLINK) {
        out |= CoreModifier::SLOW_BLINK;
    }
    if modifier.contains(Modifier::RAPID_BLINK) {
        out |= CoreModifier::RAPID_BLINK;
    }
    if modifier.contains(Modifier::REVERSED) {
        out |= CoreModifier::REVERSED;
    }
    if modifier.contains(Modifier::HIDDEN) {
        out |= CoreModifier::HIDDEN;
    }
    if modifier.contains(Modifier::CROSSED_OUT) {
        out |= CoreModifier::CROSSED_OUT;
    }
    out
}

pub(super) fn core_modifier_to_modifier(modifier: CoreModifier) -> Modifier {
    let mut out = Modifier::empty();
    if modifier.contains(CoreModifier::BOLD) {
        out |= Modifier::BOLD;
    }
    if modifier.contains(CoreModifier::DIM) {
        out |= Modifier::DIM;
    }
    if modifier.contains(CoreModifier::ITALIC) {
        out |= Modifier::ITALIC;
    }
    if modifier.contains(CoreModifier::UNDERLINED) {
        out |= Modifier::UNDERLINED;
    }
    if modifier.contains(CoreModifier::SLOW_BLINK) {
        out |= Modifier::SLOW_BLINK;
    }
    if modifier.contains(CoreModifier::RAPID_BLINK) {
        out |= Modifier::RAPID_BLINK;
    }
    if modifier.contains(CoreModifier::REVERSED) {
        out |= Modifier::REVERSED;
    }
    if modifier.contains(CoreModifier::HIDDEN) {
        out |= Modifier::HIDDEN;
    }
    if modifier.contains(CoreModifier::CROSSED_OUT) {
        out |= Modifier::CROSSED_OUT;
    }
    out
}

pub(super) fn style_to_core(style: Style) -> CoreStyle {
    let mut out = CoreStyle::default();
    if let Some(fg) = style.fg {
        out = out.fg(color_to_core(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(color_to_core(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(color_to_core(ul));
    }
    out.add_modifier = modifier_to_core(style.add_modifier);
    out.sub_modifier = modifier_to_core(style.sub_modifier);
    out
}

pub(super) fn core_style_to_style(style: CoreStyle) -> Style {
    let mut out = Style::default();
    if let Some(fg) = style.fg {
        out = out.fg(core_color_to_color(fg));
    }
    if let Some(bg) = style.bg {
        out = out.bg(core_color_to_color(bg));
    }
    if let Some(ul) = style.underline_color {
        out = out.underline_color(core_color_to_color(ul));
    }
    out.add_modifier = core_modifier_to_modifier(style.add_modifier);
    out.sub_modifier = core_modifier_to_modifier(style.sub_modifier);
    out
}

impl tui_markdown::StyleSheet for CarlosMarkdownStyleSheet {
    fn heading(&self, level: u8) -> CoreStyle {
        match level {
            1 => style_to_core(
                Style::default()
                    .fg(COLOR_TEXT)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ),
            2 => style_to_core(Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD)),
            _ => style_to_core(Style::default().fg(COLOR_TEXT)),
        }
    }

    fn code(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_TEXT))
    }

    fn link(&self) -> CoreStyle {
        style_to_core(
            Style::default()
                .fg(COLOR_GUTTER_USER)
                .add_modifier(Modifier::UNDERLINED),
        )
    }

    fn blockquote(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }

    fn heading_meta(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM).add_modifier(Modifier::DIM))
    }

    fn metadata_block(&self) -> CoreStyle {
        style_to_core(Style::default().fg(COLOR_DIM))
    }
}

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

        let plain = styled_plain_text(&segments);
        if is_fence_delimiter(&plain) {
            continue;
        }
        out.push(segments);
    }
    out
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
    if styled_plain_text(&styled_segments) == part {
        styled_segments
    } else {
        vec![StyledSegment {
            text: part.to_string(),
            style: Style::default(),
        }]
    }
}

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
                out.push(RenderedLine {
                    cells: 0,
                    text: String::new(),
                    styled_segments: Vec::new(),
                    role,
                    separator: false,
                    soft_wrap_to_next: false,
                });
            }
            in_code_fence = !in_code_fence;
            continue;
        }

        if logical.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(logical, width);

        for (i, part) in wrapped_parts.iter().enumerate() {
            let wrapped = i + 1 < wrapped_parts.len();

            out.push(RenderedLine {
                cells: visual_width(part),
                text: part.clone(),
                styled_segments: vec![StyledSegment {
                    text: part.clone(),
                    style: if matches!(role, Role::User) && in_code_fence {
                        Style::default().bg(COLOR_STEP2)
                    } else {
                        Style::default()
                    },
                }],
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
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
    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(&plain, width);
        let mut remaining: VecDeque<StyledSegment> = logical.into();

        for (i, part) in wrapped_parts.iter().enumerate() {
            let part_cells = visual_width(part);
            let wrapped = i + 1 < wrapped_parts.len();
            let styled_segments = normalize_styled_segments_for_part(
                part,
                take_styled_segments_by_cells(&mut remaining, part_cells),
            );

            out.push(RenderedLine {
                cells: part_cells,
                text: part.clone(),
                styled_segments,
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
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

    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(&plain, width);
        let mut remaining: VecDeque<StyledSegment> = logical.into();

        for (i, part) in wrapped_parts.iter().enumerate() {
            let part_cells = visual_width(part);
            let wrapped = i + 1 < wrapped_parts.len();
            let styled_segments = normalize_styled_segments_for_part(
                part,
                take_styled_segments_by_cells(&mut remaining, part_cells),
            );

            out.push(RenderedLine {
                cells: part_cells,
                text: part.clone(),
                styled_segments,
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
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

    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            out.push(RenderedLine {
                cells: 0,
                text: String::new(),
                styled_segments: Vec::new(),
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
            continue;
        }

        let wrapped_parts = wrap_natural_by_cells(&plain, width);
        let mut remaining: VecDeque<StyledSegment> = logical.into();

        for (i, part) in wrapped_parts.iter().enumerate() {
            let part_cells = visual_width(part);
            let wrapped = i + 1 < wrapped_parts.len();
            let styled_segments = normalize_styled_segments_for_part(
                part,
                take_styled_segments_by_cells(&mut remaining, part_cells),
            );

            out.push(RenderedLine {
                cells: part_cells,
                text: part.clone(),
                styled_segments,
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
}

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

    let mut count = 0usize;
    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            count += 1;
        } else {
            count += wrap_natural_count_by_cells(&plain, width);
        }
    }
    count
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
    let mut count = 0usize;
    for logical in logical_lines {
        let plain = styled_plain_text(&logical);
        if plain.is_empty() {
            count += 1;
        } else {
            count += wrap_natural_count_by_cells(&plain, width);
        }
    }
    count
}

fn fast_reasoning_summary_plain_text(line: &str) -> Option<&str> {
    let inner = line.strip_prefix("**")?.strip_suffix("**")?;
    if inner.is_empty() || inner.contains("**") {
        return None;
    }
    Some(inner.trim_end_matches(' '))
}

fn contains_terminal_escapes(text: &str) -> bool {
    text.contains('\u{1b}') || text.contains('\u{009b}')
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
        count += if skip_fence_delimiters && line.trim_matches([' ', '\t', '\r']).starts_with("```")
        {
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
        count += if skip_fence_delimiters && line.trim_matches([' ', '\t', '\r']).starts_with("```")
        {
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
