//! Diff rendering: hunk-oriented display with line numbers and colour coding.

use ratatui::style::{Modifier, Style};
use ratatui_interact::components::{DiffData, DiffLine, DiffLineType, DiffViewerStyle};

use super::models::{RenderedLine, Role, StyledSegment};
use super::text::{visual_width, wrap_natural_by_cells, wrap_natural_count_by_cells};
use super::transcript_styles::{append_wrapped_styled_logical_lines, count_styled_logical_lines};
use crate::theme::{
    COLOR_DIFF_ADD, COLOR_DIFF_HEADER, COLOR_DIFF_HUNK, COLOR_DIFF_REMOVE, COLOR_DIM,
    COLOR_STEP1, COLOR_STEP2, COLOR_STEP6, COLOR_STEP8, COLOR_TEXT,
};

// --- Diff Styles ---
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
    Style::default()
}

pub(super) fn make_diff_viewer_style() -> DiffViewerStyle {
    DiffViewerStyle {
        border_style: Style::default().fg(COLOR_DIFF_HEADER),
        line_number_style: Style::default().fg(COLOR_STEP8),
        context_style: Style::default().fg(COLOR_TEXT),
        addition_style: Style::default().fg(COLOR_DIFF_ADD),
        addition_bg: COLOR_STEP2,
        deletion_style: Style::default().fg(COLOR_DIFF_REMOVE),
        deletion_bg: COLOR_STEP2,
        inline_addition_style: Style::default()
            .fg(COLOR_STEP1)
            .bg(COLOR_DIFF_ADD)
            .add_modifier(Modifier::BOLD),
        inline_deletion_style: Style::default()
            .fg(COLOR_STEP1)
            .bg(COLOR_DIFF_REMOVE)
            .add_modifier(Modifier::BOLD),
        hunk_header_style: Style::default()
            .fg(COLOR_DIFF_HUNK)
            .add_modifier(Modifier::BOLD),
        match_style: Style::default().bg(COLOR_STEP6).fg(COLOR_TEXT),
        current_match_style: Style::default().bg(COLOR_DIFF_HUNK).fg(COLOR_STEP1),
        gutter_separator: "│",
        side_separator: "│",
    }
}

// --- Segment Helpers ---
fn diff_line_number_width(parsed: &DiffData) -> usize {
    let mut max_line = 0usize;
    for hunk in &parsed.hunks {
        max_line = max_line.max(hunk.old_start + hunk.old_count);
        max_line = max_line.max(hunk.new_start + hunk.new_count);
    }
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

// --- Logical Lines ---
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

// --- Diff Rendering ---
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

// --- Diff Counting ---
pub(super) fn count_wrapped_diff_lines(file_path: Option<&str>, diff: &str, width: usize) -> usize {
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
