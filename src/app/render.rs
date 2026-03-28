use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use ansi_to_tui::IntoText as _;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier, Style};
use ratatui_core::style::{Color as CoreColor, Modifier as CoreModifier, Style as CoreStyle};
use ratatui_interact::components::{DiffData, DiffLine, DiffLineType, DiffViewerStyle};
use ratatui_textarea::{Input as TextInput, Key as TextKey};
use tui_markdown::{
    from_str_with_options as markdown_from_str_with_options, Options as MarkdownOptions,
};

use super::context_usage::{
    context_label_reserved_cells, context_usage_label, context_usage_placeholder_label,
};
use super::perf::PerfMetrics;
use super::selection::compute_selection_range;
use super::state::ModelSettingsField;
use super::text::{
    char_to_byte_idx, count_ascii_multiline_by_cells, slice_by_cells, split_at_cells, visual_width,
    wrap_input_line, wrap_natural_by_cells, wrap_natural_count_by_cells,
};
use super::tools::strip_terminal_controls;
use super::{
    animation_tick, kitt_head_index, AppState, Message, MessageKind, RenderedLine, Role,
    StyledSegment, TerminalSize, MSG_CONTENT_X, MSG_TOP,
};
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

        let avail = width;
        let wrapped_parts = wrap_natural_by_cells(logical, avail);

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

pub(super) fn diff_line_style(line: &str) -> Style {
    if line.starts_with("@@") {
        return Style::default()
            .fg(COLOR_DIFF_HUNK)
            .add_modifier(Modifier::BOLD);
    }
    if (line.starts_with("+++") || line.starts_with("---")) && line.len() > 3 {
        return Style::default().fg(COLOR_DIFF_HEADER);
    }
    if line.starts_with("diff --git ")
        || line.starts_with("index ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
    {
        return Style::default()
            .fg(COLOR_DIFF_HEADER)
            .add_modifier(Modifier::DIM);
    }
    if line.starts_with('+') && !line.starts_with("+++") {
        return Style::default().fg(COLOR_DIFF_ADD);
    }
    if line.starts_with('-') && !line.starts_with("---") {
        return Style::default().fg(COLOR_DIFF_REMOVE);
    }
    Style::default().fg(COLOR_TEXT)
}

pub(super) fn make_diff_viewer_style() -> DiffViewerStyle {
    DiffViewerStyle {
        border_style: Style::default().fg(COLOR_STEP6),
        line_number_style: Style::default().fg(COLOR_DIM),
        context_style: Style::default().fg(COLOR_TEXT),
        addition_style: Style::default().fg(COLOR_DIFF_ADD),
        addition_bg: Color::Rgb(22, 41, 29),
        deletion_style: Style::default().fg(COLOR_DIFF_REMOVE),
        deletion_bg: Color::Rgb(52, 25, 38),
        inline_addition_style: Style::default().fg(COLOR_STEP1).bg(COLOR_DIFF_ADD),
        inline_deletion_style: Style::default().fg(COLOR_STEP1).bg(COLOR_DIFF_REMOVE),
        hunk_header_style: Style::default()
            .fg(COLOR_DIFF_HUNK)
            .add_modifier(Modifier::BOLD),
        match_style: Style::default().bg(COLOR_STEP6).fg(COLOR_TEXT),
        current_match_style: Style::default().bg(COLOR_PRIMARY).fg(COLOR_STEP1),
        gutter_separator: "│",
        side_separator: "│",
    }
}

fn diff_line_number_width(parsed: &DiffData) -> usize {
    let max_line = parsed
        .hunks
        .iter()
        .map(|h| h.old_start + h.old_count.max(h.new_count))
        .max()
        .unwrap_or(1);
    max_line.to_string().len().max(3)
}

fn push_styled_text(segments: &mut Vec<StyledSegment>, text: String, style: Style) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = segments.last_mut() {
        if last.style == style {
            last.text.push_str(&text);
            return;
        }
    }
    segments.push(StyledSegment { text, style });
}

fn build_diff_transcript_line_segments(
    line: &DiffLine,
    line_num_width: usize,
    style: &DiffViewerStyle,
) -> Vec<StyledSegment> {
    let mut segments = Vec::new();
    let old_num = line
        .old_line_num
        .map(|n| format!("{:>width$}", n, width = line_num_width))
        .unwrap_or_else(|| " ".repeat(line_num_width));
    let new_num = line
        .new_line_num
        .map(|n| format!("{:>width$}", n, width = line_num_width))
        .unwrap_or_else(|| " ".repeat(line_num_width));
    push_styled_text(&mut segments, old_num, style.line_number_style);
    push_styled_text(&mut segments, " ".to_string(), style.line_number_style);
    push_styled_text(&mut segments, new_num, style.line_number_style);
    push_styled_text(
        &mut segments,
        format!(" {} ", style.gutter_separator),
        style.line_number_style,
    );

    let (prefix, content_style) = match line.line_type {
        DiffLineType::Context => (" ", style.context_style),
        DiffLineType::Addition => (
            "+",
            style
                .addition_style
                .patch(Style::default().bg(style.addition_bg)),
        ),
        DiffLineType::Deletion => (
            "-",
            style
                .deletion_style
                .patch(Style::default().bg(style.deletion_bg)),
        ),
        DiffLineType::HunkHeader => ("@", style.hunk_header_style),
    };
    push_styled_text(&mut segments, prefix.to_string(), content_style);
    push_styled_text(&mut segments, line.content.clone(), content_style);
    segments
}

fn diff_transcript_logical_lines(
    file_path: Option<&str>,
    diff: &str,
) -> Option<Vec<Vec<StyledSegment>>> {
    let parsed = DiffData::from_unified_diff(diff);
    if parsed.hunks.is_empty() {
        return None;
    }

    let diff_path = file_path
        .filter(|p| !p.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| parsed.new_path.clone())
        .or_else(|| parsed.old_path.clone());
    let style = make_diff_viewer_style();
    let line_num_width = diff_line_number_width(&parsed);
    let mut logical_lines = Vec::new();

    for (idx, hunk) in parsed.hunks.iter().enumerate() {
        if idx > 0 {
            logical_lines.push(Vec::new());
        }

        if let Some(path) = diff_path.as_deref() {
            if !path.is_empty() {
                logical_lines.push(vec![StyledSegment {
                    text: path.to_string(),
                    style: Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                }]);
            }
        }

        let hunk_label = format!(
            "Hunk {}/{}  old {}..{} -> new {}..{}",
            idx + 1,
            parsed.hunks.len(),
            hunk.old_start,
            hunk.old_start + hunk.old_count.saturating_sub(1),
            hunk.new_start,
            hunk.new_start + hunk.new_count.saturating_sub(1)
        );
        logical_lines.push(vec![StyledSegment {
            text: hunk_label,
            style: Style::default()
                .fg(COLOR_DIFF_HUNK)
                .add_modifier(Modifier::BOLD),
        }]);

        for line in &hunk.lines {
            if line.line_type == DiffLineType::HunkHeader {
                continue;
            }
            logical_lines.push(build_diff_transcript_line_segments(
                line,
                line_num_width,
                &style,
            ));
        }
    }

    Some(logical_lines)
}

pub(super) fn append_diff_viewer_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    file_path: Option<&str>,
    diff: &str,
    width: usize,
) -> bool {
    let Some(logical_lines) = diff_transcript_logical_lines(file_path, diff) else {
        return false;
    };
    append_wrapped_styled_logical_lines(out, role, logical_lines, width);
    true
}

pub(super) fn append_wrapped_diff_lines(
    out: &mut Vec<RenderedLine>,
    role: Role,
    file_path: Option<&str>,
    diff: &str,
    width: usize,
) {
    if append_diff_viewer_lines(out, role, file_path, diff, width) {
        return;
    }

    if width < 8 {
        return;
    }

    if let Some(path) = file_path {
        if !path.is_empty() {
            out.push(RenderedLine {
                cells: visual_width(path),
                text: path.to_string(),
                styled_segments: vec![StyledSegment {
                    text: path.to_string(),
                    style: Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                }],
                role,
                separator: false,
                soft_wrap_to_next: false,
            });
        }
    }

    for logical in diff.split('\n') {
        let line_style = diff_line_style(logical);
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
                    style: line_style,
                }],
                role,
                separator: false,
                soft_wrap_to_next: wrapped,
            });
        }
    }
}

pub(super) fn read_summary_path(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix("→ Read")?.trim_start();
    if rest.is_empty() {
        return Some("");
    }
    let path = rest.split_once(" [").map(|(p, _)| p).unwrap_or(rest).trim();
    Some(path)
}

pub(super) fn parse_read_summary(text: &str) -> Option<(&str, usize)> {
    let path = read_summary_path(text)?;
    if let Some((base, count)) = path.rsplit_once(" ×") {
        let parsed = count.parse::<usize>().ok()?;
        return Some((base.trim_end(), parsed.max(1)));
    }
    Some((path, 1))
}

pub(super) fn format_read_summary_with_count(path: &str, count: usize) -> String {
    let base = if path.is_empty() {
        "→ Read".to_string()
    } else {
        format!("→ Read {path}")
    };
    if count > 1 {
        format!("{base} ×{count}")
    } else {
        base
    }
}

#[cfg(test)]
pub(super) fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    build_rendered_lines_with_hidden(messages, width, None)
}

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
        if !message_has_visible_content(msg) {
            continue;
        }
        append_rendered_block_for_message(&mut out, previous_visible, msg, width);
        previous_visible = Some(msg);
    }

    out
}

pub(super) fn build_rendered_block_for_message(
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) -> Vec<RenderedLine> {
    let mut out = Vec::new();
    append_rendered_block_for_message(&mut out, previous_visible, msg, width);
    out
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn count_rendered_block_for_message(
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) -> usize {
    if !message_has_visible_content(msg) {
        return 0;
    }

    let mut count = 0usize;
    if let Some(prev) = previous_visible {
        if should_insert_separator_between(prev, msg) {
            count += 1;
        }
    }

    count
        + match msg.kind {
            MessageKind::Diff => {
                count_wrapped_diff_lines(msg.file_path.as_deref(), &msg.text, width)
            }
            MessageKind::Plain => match msg.role {
                Role::Assistant | Role::Reasoning => {
                    count_wrapped_markdown_lines(msg.role, &msg.text, width)
                }
                Role::ToolOutput => count_wrapped_ansi_lines(msg.role, &msg.text, width),
                _ => count_wrapped_message_lines(msg.role, &msg.text, width),
            },
        }
}

pub(super) fn count_rendered_block_for_message_cached<'a>(
    cache: &mut RenderCountCache<'a>,
    previous_visible: Option<&Message>,
    msg: &'a Message,
    width: usize,
) -> usize {
    if !message_has_visible_content(msg) {
        return 0;
    }

    let mut count = 0usize;
    if let Some(prev) = previous_visible {
        if should_insert_separator_between(prev, msg) {
            count += 1;
        }
    }

    count
        + match msg.kind {
            MessageKind::Diff => {
                count_wrapped_diff_lines(msg.file_path.as_deref(), &msg.text, width)
            }
            MessageKind::Plain => match msg.role {
                Role::Assistant | Role::Reasoning => {
                    count_wrapped_markdown_lines(msg.role, &msg.text, width)
                }
                Role::ToolOutput => {
                    count_wrapped_ansi_lines_cached(cache, msg.role, &msg.text, width)
                }
                _ => count_wrapped_message_lines_cached(cache, msg.role, &msg.text, width),
            },
        }
}

pub(super) fn append_rendered_block_for_message(
    out: &mut Vec<RenderedLine>,
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) {
    if !message_has_visible_content(msg) {
        return;
    }
    if let Some(prev) = previous_visible {
        if should_insert_separator_between(prev, msg) {
            out.push(RenderedLine {
                text: String::new(),
                styled_segments: Vec::new(),
                role: Role::System,
                separator: true,
                cells: 0,
                soft_wrap_to_next: false,
            });
        }
    }

    match msg.kind {
        MessageKind::Diff => {
            append_wrapped_diff_lines(out, msg.role, msg.file_path.as_deref(), &msg.text, width)
        }
        MessageKind::Plain => match msg.role {
            Role::Assistant | Role::Reasoning => {
                append_wrapped_markdown_lines(out, msg.role, &msg.text, width);
            }
            Role::ToolOutput => append_wrapped_ansi_lines(out, msg.role, &msg.text, width),
            _ => append_wrapped_message_lines(out, msg.role, &msg.text, width),
        },
    }
}

fn count_wrapped_message_lines(role: Role, text: &str, width: usize) -> usize {
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

fn count_wrapped_message_lines_cached<'a>(
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

fn count_wrapped_markdown_lines(role: Role, text: &str, width: usize) -> usize {
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
fn count_wrapped_ansi_lines(role: Role, text: &str, width: usize) -> usize {
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

fn count_wrapped_ansi_lines_cached<'a>(
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

fn count_wrapped_diff_lines(file_path: Option<&str>, diff: &str, width: usize) -> usize {
    if width < 8 {
        return 0;
    }

    let logical_lines = match diff_transcript_logical_lines(file_path, diff) {
        Some(logical_lines) => logical_lines,
        None => {
            let mut count = 0usize;
            if let Some(path) = file_path {
                if !path.is_empty() {
                    count += 1;
                }
            }
            for logical in diff.split('\n') {
                if logical.is_empty() {
                    count += 1;
                } else {
                    count += wrap_natural_count_by_cells(logical, width);
                }
            }
            return count;
        }
    };
    count_styled_logical_lines(logical_lines, width)
}

fn count_styled_logical_lines(logical_lines: Vec<Vec<StyledSegment>>, width: usize) -> usize {
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

fn message_has_visible_content(msg: &Message) -> bool {
    match msg.kind {
        MessageKind::Diff => !msg.text.trim().is_empty(),
        MessageKind::Plain => !msg.text.trim().is_empty(),
    }
}

fn should_insert_separator_between(prev: &Message, next: &Message) -> bool {
    let _ = (prev, next);
    true
}

pub(super) fn transcript_content_width(size: TerminalSize) -> usize {
    size.width.saturating_sub(MSG_CONTENT_X + 1)
}

#[derive(Debug, Clone)]
pub(super) struct InputLayout {
    pub(super) msg_bottom: usize, // 1-based; 0 means no transcript row is available
    pub(super) input_top: usize,  // 1-based
    pub(super) input_height: usize, // rows
    pub(super) text_width: usize, // cells available for input text
    pub(super) visible_lines: Vec<String>,
    pub(super) cursor_x: usize, // 0-based terminal column
    pub(super) cursor_y: usize, // 0-based terminal row
}

pub(super) fn input_cursor_visual_position(
    line: &str,
    cursor_col_chars: usize,
    width: usize,
) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }

    let cursor_byte = char_to_byte_idx(line, cursor_col_chars).min(line.len());
    let prefix = &line[..cursor_byte];
    let wrapped_prefix = wrap_input_line(prefix, width);
    let row = wrapped_prefix.len().saturating_sub(1);
    let col = wrapped_prefix
        .last()
        .map(|part| visual_width(part))
        .unwrap_or(0);
    (row, col)
}

pub(super) fn textarea_input_from_code(code: KeyCode, modifiers: KeyModifiers) -> TextInput {
    let key = match code {
        KeyCode::Char(c) => TextKey::Char(c),
        KeyCode::Backspace => TextKey::Backspace,
        KeyCode::Enter => TextKey::Enter,
        KeyCode::Left => TextKey::Left,
        KeyCode::Right => TextKey::Right,
        KeyCode::Up => TextKey::Up,
        KeyCode::Down => TextKey::Down,
        KeyCode::Tab => TextKey::Tab,
        KeyCode::Delete => TextKey::Delete,
        KeyCode::Home => TextKey::Home,
        KeyCode::End => TextKey::End,
        KeyCode::PageUp => TextKey::PageUp,
        KeyCode::PageDown => TextKey::PageDown,
        KeyCode::Esc => TextKey::Esc,
        KeyCode::F(n) => TextKey::F(n),
        _ => TextKey::Null,
    };

    TextInput {
        key,
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        alt: modifiers.contains(KeyModifiers::ALT),
        shift: modifiers.contains(KeyModifiers::SHIFT),
    }
}

pub(super) fn textarea_input_from_key(k: crossterm::event::KeyEvent) -> TextInput {
    textarea_input_from_code(k.code, k.modifiers)
}

pub(super) fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn is_newline_enter(mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::ALT)
}

pub(super) fn compute_input_layout(app: &AppState, size: TerminalSize) -> InputLayout {
    let text_width = transcript_content_width(size);
    let mut wrapped = Vec::new();
    let lines = app.input.lines();

    let (cursor_row, cursor_col_chars) = app.input.cursor();
    let mut cursor_wrapped_row = 0usize;
    let mut cursor_wrapped_col = 0usize;
    let mut cursor_set = false;

    for (row, line) in lines.iter().enumerate() {
        let wrapped_line = wrap_input_line(line, text_width);

        if row < cursor_row {
            cursor_wrapped_row += wrapped_line.len();
        } else if row == cursor_row {
            let (line_row, line_col) =
                input_cursor_visual_position(line, cursor_col_chars, text_width);
            cursor_wrapped_row += line_row;
            cursor_wrapped_col = line_col;
            cursor_set = true;
        }

        wrapped.extend(wrapped_line);
    }

    if wrapped.is_empty() {
        wrapped.push(String::new());
    }

    let mut max_input_rows = 8usize.min(size.height.max(1));
    if size.height > 1 {
        max_input_rows = max_input_rows.min(size.height - 1);
    }

    let input_height = wrapped.len().clamp(1, max_input_rows.max(1));

    if !cursor_set {
        cursor_wrapped_row = wrapped.len().saturating_sub(1);
        cursor_wrapped_col = wrapped.last().map(|line| visual_width(line)).unwrap_or(0);
    }

    let mut visible_start = wrapped.len().saturating_sub(input_height);
    if cursor_wrapped_row < visible_start {
        visible_start = cursor_wrapped_row;
    }
    if cursor_wrapped_row >= visible_start + input_height {
        visible_start = cursor_wrapped_row + 1 - input_height;
    }

    let visible_end = (visible_start + input_height).min(wrapped.len());
    let mut visible_lines = wrapped[visible_start..visible_end].to_vec();
    while visible_lines.len() < input_height {
        visible_lines.insert(0, String::new());
    }

    let input_top = size.height + 1 - input_height;
    let msg_bottom = input_top.saturating_sub(2);
    let cursor_visual_row = cursor_wrapped_row.saturating_sub(visible_start);
    let cursor_x = MSG_CONTENT_X + cursor_wrapped_col;
    let cursor_y = input_top.saturating_sub(1) + cursor_visual_row.min(input_height - 1);

    InputLayout {
        msg_bottom,
        input_top,
        input_height,
        text_width,
        visible_lines,
        cursor_x,
        cursor_y,
    }
}

pub(super) fn last_assistant_message(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, Role::Assistant) && !m.text.is_empty())
        .map(|m| m.text.as_str())
}

pub(super) fn draw_str(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    text: &str,
    style: Style,
    max_width: usize,
) {
    if text.is_empty() || max_width == 0 {
        return;
    }
    if let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) {
        buf.set_stringn(x, y, text, max_width, style);
    }
}

pub(super) fn fill_rect(buf: &mut Buffer, x: usize, y: usize, w: usize, h: usize, style: Style) {
    if w == 0 || h == 0 {
        return;
    }
    if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
        u16::try_from(x),
        u16::try_from(y),
        u16::try_from(w),
        u16::try_from(h),
    ) {
        let blank = " ".repeat(w as usize);
        for row in 0..h {
            buf.set_stringn(x, y + row, &blank, w as usize, style);
        }
    }
}

pub(super) fn draw_rendered_line(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    max_width: usize,
    line: &RenderedLine,
    base_style: Style,
    selection: Option<(usize, usize)>,
) {
    if max_width == 0 || line.cells == 0 {
        return;
    }

    if !line.styled_segments.is_empty() {
        draw_str(buf, x, y, &line.text, base_style, max_width);
    }

    let mut draw_x = x;
    let mut col = 0usize;

    let mut render_segment = |text: &str, seg_style: Style, draw_x: &mut usize, col: &mut usize| {
        if *draw_x >= x + max_width || text.is_empty() {
            return;
        }

        let seg_cells = visual_width(text);
        if seg_cells == 0 {
            return;
        }

        let style = base_style.patch(seg_style);
        let seg_start = *col;
        let seg_end = seg_start + seg_cells;

        let mut draw_piece = |piece: &str, piece_style: Style, draw_x: &mut usize| {
            if piece.is_empty() || *draw_x >= x + max_width {
                return;
            }
            let rem = max_width.saturating_sub(*draw_x - x);
            draw_str(buf, *draw_x, y, piece, piece_style, rem);
            *draw_x += visual_width(piece);
        };

        if let Some((sel_start, sel_end)) = selection {
            if sel_end <= seg_start || sel_start >= seg_end {
                draw_piece(text, style, draw_x);
            } else {
                let local_start = sel_start.saturating_sub(seg_start).min(seg_cells);
                let local_end = sel_end.saturating_sub(seg_start).min(seg_cells);

                let before = slice_by_cells(text, 0, local_start);
                let selected = slice_by_cells(text, local_start, local_end);
                let after = slice_by_cells(text, local_end, seg_cells);

                draw_piece(&before, style, draw_x);
                draw_piece(&selected, style.fg(COLOR_TEXT).bg(COLOR_STEP8), draw_x);
                draw_piece(&after, style, draw_x);
            }
        } else {
            draw_piece(text, style, draw_x);
        }

        *col = seg_end;
    };

    if line.styled_segments.is_empty() {
        render_segment(&line.text, Style::default(), &mut draw_x, &mut col);
        return;
    }

    for seg in &line.styled_segments {
        if draw_x >= x + max_width {
            break;
        }
        render_segment(&seg.text, seg.style, &mut draw_x, &mut col);
    }

    // Some markdown renderers occasionally leave a trailing token outside
    // styled spans; render the uncovered tail from canonical line text.
    if col < line.cells && draw_x < x + max_width {
        let tail = slice_by_cells(&line.text, col, line.cells);
        render_segment(&tail, Style::default(), &mut draw_x, &mut col);
    }
}

pub(super) fn draw_help_overlay(buf: &mut Buffer, size: TerminalSize) {
    if !(size.height > 10 && size.width > 44) {
        return;
    }

    let box_w = (size.width - 8).min(74);
    let box_h = 10usize;
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┏", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┓", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "┗", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┛", Style::default().fg(COLOR_STEP7), 1);

    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "┃", Style::default().fg(COLOR_STEP7), 1);
    }

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        "Help",
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + box_w.saturating_sub(8),
        start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 3,
        "Enter send/steer  Shift/Alt+Enter newline",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 4,
        "Ctrl+Y copy selection or last answer",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 5,
        "Home/End jump transcript  Ctrl+M model/effort/summary",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 6,
        "Wheel scroll, drag to select, release to copy",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 7,
        "? toggle this help",
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

pub(super) fn draw_model_settings_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    if !(size.height > 14 && size.width > 56) {
        return;
    }

    let box_w = (size.width - 10).min(80);
    let box_h = 12usize;
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┏", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┓", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "┗", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┛", Style::default().fg(COLOR_STEP7), 1);
    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "┃", Style::default().fg(COLOR_STEP7), 1);
    }

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        "Model / Thinking / Summary",
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + box_w.saturating_sub(8),
        start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );

    let model_style = if matches!(app.model_settings_field, ModelSettingsField::Model) {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    };
    let effort_style = if matches!(app.model_settings_field, ModelSettingsField::Effort) {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    };
    let summary_style = if matches!(app.model_settings_field, ModelSettingsField::Summary) {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    };
    let model_value = app.model_settings_model_value();
    let effort_value = app.model_settings_effort_value();
    let summary_value = app.model_settings_summary_value();

    draw_str(
        buf,
        start_x + 3,
        start_y + 3,
        "Model",
        Style::default().fg(COLOR_DIM),
        8,
    );
    draw_str(
        buf,
        start_x + 12,
        start_y + 3,
        model_value,
        model_style,
        box_w.saturating_sub(16),
    );

    draw_str(
        buf,
        start_x + 3,
        start_y + 5,
        "Thinking",
        Style::default().fg(COLOR_DIM),
        8,
    );
    draw_str(
        buf,
        start_x + 12,
        start_y + 5,
        effort_value,
        effort_style,
        box_w.saturating_sub(16),
    );

    draw_str(
        buf,
        start_x + 3,
        start_y + 7,
        "Summary",
        Style::default().fg(COLOR_DIM),
        8,
    );
    draw_str(
        buf,
        start_x + 12,
        start_y + 7,
        summary_value,
        summary_style,
        box_w.saturating_sub(16),
    );

    draw_str(
        buf,
        start_x + 3,
        start_y + 9,
        "Tab switch field, arrows adjust, Enter apply",
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

pub(super) fn draw_approval_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    let Some(approval) = app.pending_approval.as_ref() else {
        return;
    };
    if size.width < 36 || size.height < 10 {
        return;
    }

    let inner_w = size.width.saturating_sub(12).min(92);
    let mut detail_lines = Vec::new();
    for line in &approval.detail_lines {
        let wrapped = wrap_natural_by_cells(line, inner_w.saturating_sub(6).max(8));
        if wrapped.is_empty() {
            detail_lines.push(String::new());
        } else {
            detail_lines.extend(wrapped);
        }
    }
    if detail_lines.is_empty() {
        detail_lines.push("No additional detail provided.".to_string());
    }

    let mut footer = vec!["y accept".to_string(), "n decline".to_string()];
    if approval.can_accept_for_session {
        footer.push("s accept session".to_string());
    }
    if approval.can_cancel {
        footer.push("c cancel turn".to_string());
    }
    let footer_text = footer.join("  ");

    let max_body_lines = size.height.saturating_sub(7);
    if detail_lines.len() > max_body_lines {
        detail_lines.truncate(max_body_lines);
        if let Some(last) = detail_lines.last_mut() {
            *last = "…".to_string();
        }
    }

    let box_w = inner_w;
    let box_h = (detail_lines.len() + 5).min(size.height.saturating_sub(2));
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┏", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┓", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "┗", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┛", Style::default().fg(COLOR_STEP7), 1);
    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "┃", Style::default().fg(COLOR_STEP7), 1);
    }

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        &approval.title,
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 2,
        &approval.method,
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );

    for (i, line) in detail_lines.iter().enumerate() {
        draw_str(
            buf,
            start_x + 3,
            start_y + 3 + i,
            line,
            Style::default().fg(COLOR_TEXT),
            box_w.saturating_sub(6),
        );
    }

    draw_str(
        buf,
        start_x + 3,
        bottom.saturating_sub(1),
        &footer_text,
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

pub(super) fn draw_perf_overlay(buf: &mut Buffer, size: TerminalSize, perf: &PerfMetrics) {
    let lines = perf.overlay_lines();
    if lines.is_empty() || size.width < 44 || size.height < lines.len() + 4 {
        return;
    }

    let inner_w = lines
        .iter()
        .map(|line| visual_width(line))
        .max()
        .unwrap_or(0)
        .min(size.width.saturating_sub(6));
    if inner_w == 0 {
        return;
    }

    let box_w = inner_w + 4;
    let box_h = lines.len() + 2;
    let start_x = size.width.saturating_sub(box_w + 2);
    let start_y = 1usize;
    if start_y + box_h > size.height {
        return;
    }

    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP1),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;

    draw_str(buf, left, top, "┌", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, top, "┐", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, left, bottom, "└", Style::default().fg(COLOR_STEP7), 1);
    draw_str(buf, right, bottom, "┘", Style::default().fg(COLOR_STEP7), 1);

    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, x, bottom, "─", Style::default().fg(COLOR_STEP7), 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "│", Style::default().fg(COLOR_STEP7), 1);
        draw_str(buf, right, y, "│", Style::default().fg(COLOR_STEP7), 1);
    }

    for (i, line) in lines.iter().enumerate() {
        draw_str(
            buf,
            start_x + 2,
            start_y + 1 + i,
            line,
            Style::default().fg(COLOR_DIM),
            inner_w,
        );
    }
}

pub(super) fn render_main_view(frame: &mut ratatui::Frame<'_>, app: &mut AppState) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let input_layout_started = Instant::now();
    let input_layout = compute_input_layout(app, size);
    if let Some(perf) = app.perf.as_mut() {
        perf.input_layout.push(input_layout_started.elapsed());
    }
    let msg_top = MSG_TOP;
    let msg_bottom = input_layout.msg_bottom;
    let msg_height = if msg_bottom >= msg_top {
        msg_bottom - msg_top + 1
    } else {
        0
    };
    let msg_width = transcript_content_width(size);

    let total_lines = app.rendered_line_count();
    let max_scroll = total_lines.saturating_sub(msg_height);
    if app.scroll_top > max_scroll {
        app.scroll_top = max_scroll;
    }
    if app.auto_follow_bottom && max_scroll > 0 {
        app.scroll_top = max_scroll;
    }
    if msg_height > 0 && total_lines > 0 {
        let end_idx = (app.scroll_top + msg_height)
            .saturating_sub(1)
            .min(total_lines.saturating_sub(1));
        app.ensure_rendered_range_materialized(app.scroll_top, end_idx);
    }

    let buf = frame.buffer_mut();
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    if msg_height > 0 {
        fill_rect(
            buf,
            0,
            msg_top - 1,
            size.width,
            msg_height,
            Style::default().bg(COLOR_STEP2),
        );
    }

    for i in 0..msg_height {
        let line_idx = app.scroll_top + i;
        let row_1b = msg_top + i;
        let y = row_1b - 1;

        let line_opt = app.rendered_line_at(line_idx);
        if let Some(line) = line_opt {
            if !line.separator {
                let gutter_symbol = role_gutter_symbol(line.role);
                draw_str(
                    buf,
                    0,
                    y,
                    gutter_symbol,
                    Style::default()
                        .fg(role_gutter_fg(line.role))
                        .add_modifier(Modifier::BOLD),
                    1,
                );
            }
        }

        if msg_width == 0 {
            continue;
        }

        let Some(line) = line_opt else {
            continue;
        };
        fill_rect(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            1,
            Style::default().bg(role_row_bg(line.role)),
        );
        if line.separator {
            let sep = "─".repeat(msg_width);
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                &sep,
                Style::default().fg(COLOR_STEP6),
                msg_width,
            );
            continue;
        }

        let mut base_style = Style::default().fg(role_fg(line.role));
        if matches!(line.role, Role::Reasoning) {
            base_style = base_style.add_modifier(Modifier::DIM);
        } else if matches!(line.role, Role::Commentary) {
            base_style = base_style.add_modifier(Modifier::DIM | Modifier::ITALIC);
        }

        let selection_range = app
            .selection
            .and_then(|sel| compute_selection_range(sel, line_idx, line.cells))
            .map(|(start, end)| (start.min(line.cells), end.min(line.cells)));

        draw_rendered_line(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            line,
            base_style,
            selection_range,
        );
    }

    if input_layout.input_top > 1 {
        let sep_y = input_layout.input_top - 2;
        if size.width > 0 {
            let working = app.active_turn_id.is_some();
            let ralph_mode = app.ralph.is_some();
            const RALPH_MODE_LABEL: &str = "RALPH MODE";
            let line_len = size.width.saturating_sub(1);
            let context_label = app
                .context_usage
                .map(context_usage_label)
                .unwrap_or_else(|| context_usage_placeholder_label().to_string());
            let model_label = app.runtime_settings_label();
            let has_context_usage = app.context_usage.is_some();
            let has_runtime_settings = app.has_runtime_settings();
            let runtime_settings_pending = app.runtime_settings_pending();
            let ralph_label_cells = if ralph_mode {
                visual_width(RALPH_MODE_LABEL) + 1
            } else {
                0
            };
            let model_label_cells = visual_width(&model_label);
            let reserved_label_cells = context_label_reserved_cells(Some(&context_label))
                + 1
                + model_label_cells
                + ralph_label_cells;
            let context_label_cells = visual_width(&context_label);
            let can_reserve_label_area = reserved_label_cells + 1 < line_len;
            let label_area_start = if can_reserve_label_area {
                line_len - reserved_label_cells
            } else {
                line_len
            };
            let anim_end = if can_reserve_label_area {
                label_area_start.saturating_sub(1)
            } else {
                line_len
            };
            let tick = animation_tick();
            let head = if anim_end > 0 {
                kitt_head_index(anim_end, tick)
            } else {
                0
            };
            if anim_end > 0 {
                if app.rewind_mode {
                    let sep = "━".repeat(anim_end);
                    draw_str(
                        buf,
                        0,
                        sep_y,
                        &sep,
                        Style::default().fg(COLOR_DIFF_REMOVE),
                        anim_end,
                    );
                } else if working {
                    for x in 0..anim_end {
                        let dist = head.abs_diff(x);
                        draw_str(
                            buf,
                            x,
                            sep_y,
                            "━",
                            Style::default().fg(kitt_color_for_distance(dist, ralph_mode)),
                            1,
                        );
                    }
                } else {
                    let sep = "━".repeat(anim_end);
                    draw_str(
                        buf,
                        0,
                        sep_y,
                        &sep,
                        Style::default().fg(if ralph_mode {
                            COLOR_GUTTER_AGENT_THINKING
                        } else {
                            COLOR_GUTTER_USER
                        }),
                        anim_end,
                    );
                }
            }

            if can_reserve_label_area && context_label_cells > 0 {
                let context_x = line_len.saturating_sub(context_label_cells);
                let model_x = context_x.saturating_sub(model_label_cells + 1);
                draw_str(
                    buf,
                    context_x,
                    sep_y,
                    &context_label,
                    Style::default().fg(if has_context_usage {
                        COLOR_STEP8
                    } else {
                        COLOR_STEP7
                    }),
                    context_label_cells,
                );
                draw_str(
                    buf,
                    model_x,
                    sep_y,
                    &model_label,
                    Style::default().fg(if runtime_settings_pending {
                        COLOR_DIFF_HUNK
                    } else if has_runtime_settings {
                        COLOR_STEP8
                    } else {
                        COLOR_STEP7
                    }),
                    model_label_cells,
                );

                if ralph_mode {
                    let ralph_label_cells = visual_width(RALPH_MODE_LABEL);
                    let ralph_x = model_x.saturating_sub(ralph_label_cells + 1);
                    draw_str(
                        buf,
                        ralph_x,
                        sep_y,
                        RALPH_MODE_LABEL,
                        Style::default()
                            .fg(COLOR_GUTTER_AGENT_THINKING)
                            .add_modifier(Modifier::BOLD),
                        ralph_label_cells,
                    );
                }
            }
        }
    }

    let ralph_mode = app.ralph.is_some();
    fill_rect(
        buf,
        0,
        input_layout.input_top.saturating_sub(1),
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP3),
    );
    for i in 0..input_layout.input_height {
        let y = input_layout.input_top.saturating_sub(1) + i;
        let input_gutter_color = if app.rewind_mode {
            COLOR_DIFF_REMOVE
        } else if ralph_mode {
            COLOR_GUTTER_AGENT_THINKING
        } else {
            COLOR_GUTTER_USER
        };
        draw_str(
            buf,
            0,
            y,
            ">",
            Style::default()
                .fg(input_gutter_color)
                .add_modifier(Modifier::BOLD),
            1,
        );
        if let Some(line) = input_layout.visible_lines.get(i) {
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }

    if app.show_help {
        draw_help_overlay(buf, size);
    }
    if let Some(perf) = app.perf.as_ref() {
        if perf.show_overlay {
            draw_perf_overlay(buf, size, perf);
        }
    }
    if app.show_model_settings {
        draw_model_settings_overlay(buf, size, app);
    }
    if app.pending_approval.is_some() {
        draw_approval_overlay(buf, size, app);
    }

    let (cursor_x, cursor_y) = if app.pending_approval.is_some() {
        (0, size.height.saturating_sub(1))
    } else if app.show_model_settings {
        let box_w = (size.width.saturating_sub(10)).min(80);
        let box_h = 12usize;
        let start_x = (size.width.saturating_sub(box_w)) / 2;
        let start_y = (size.height.saturating_sub(box_h)) / 2;
        let x = match app.model_settings_field {
            ModelSettingsField::Model => {
                start_x + 12 + visual_width(app.model_settings_model_value())
            }
            ModelSettingsField::Effort => {
                start_x + 12 + visual_width(app.model_settings_effort_value())
            }
            ModelSettingsField::Summary => {
                start_x + 12 + visual_width(app.model_settings_summary_value())
            }
        };
        let y = match app.model_settings_field {
            ModelSettingsField::Model => start_y + 3,
            ModelSettingsField::Effort => start_y + 5,
            ModelSettingsField::Summary => start_y + 7,
        };
        (x, y)
    } else {
        (input_layout.cursor_x, input_layout.cursor_y)
    };
    let cursor_x = cursor_x.min(size.width.saturating_sub(2));
    let cursor_y = cursor_y.min(size.height.saturating_sub(1));
    frame.set_cursor_position((cursor_x as u16, cursor_y as u16));
}
