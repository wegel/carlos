//! Draw one tool label with its icon, diff counts, failure, and hidden-row count.

use ratatui::style::{Color, Modifier, Style};

use super::models::{Message, MessageKind, RenderedLine, Role, StyledSegment};
use super::text::{split_at_cells, visual_width, wrap_natural_by_cells};
use crate::theme::{
    COLOR_DIFF_ADD, COLOR_DIFF_REMOVE, COLOR_GUTTER_TOOL_CALL, COLOR_STEP8, COLOR_TOOL_EDIT,
    COLOR_TOOL_READ, COLOR_TOOL_SEARCH, COLOR_TOOL_TEXT,
};

/// Read the saved tool label rather than trying to infer it from output text.
pub(super) fn tool_label(message: &Message) -> &str {
    message
        .tool_output
        .as_ref()
        .map(|tool| tool.summary.as_str())
        .unwrap_or_else(|| message.text.lines().next().unwrap_or("Tool output"))
}

/// Use the backend failure flag for both full and folded rows.
pub(super) fn tool_failed(message: &Message) -> bool {
    message.tool_output.as_ref().is_some_and(|tool| tool.failed)
}

/// Choose the tool glyph and let failures override its color.
pub(super) fn tool_gutter(message: &Message) -> (&'static str, Color) {
    let label = tool_label(message);
    let (icon, color) = if message.kind == MessageKind::Diff
        || (label.starts_with("Δ") || label.starts_with("←"))
    {
        ("Δ", COLOR_TOOL_EDIT)
    } else if label.starts_with("→ Read") || label.starts_with("→ Inspect") {
        ("→", COLOR_TOOL_READ)
    } else if label.starts_with("✱ Search") || label.starts_with("→ List") {
        ("✱", COLOR_TOOL_SEARCH)
    } else {
        ("$", COLOR_GUTTER_TOOL_CALL)
    };
    (
        icon,
        if tool_failed(message) {
            COLOR_DIFF_REMOVE
        } else {
            color
        },
    )
}

fn tool_arguments(label: &str) -> &str {
    for prefix in [
        "run ",
        "→ Read ",
        "→ Inspect ",
        "→ List ",
        "✱ Search ",
        "Δ Diff ",
        "← Edit ",
    ] {
        if let Some(arguments) = label.strip_prefix(prefix) {
            return arguments;
        }
    }
    label
}

fn split_diff_counts(label: &str) -> Option<(&str, String, String)> {
    let (path, counts) = label.rsplit_once(" (+")?;
    let (added, removed) = counts.strip_suffix(')')?.split_once(", -")?;
    if ![added, removed]
        .iter()
        .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    Some((path, format!(" +{added}"), format!(" −{removed}")))
}

fn push_segment(segments: &mut Vec<StyledSegment>, text: &str, style: Style) {
    if !text.is_empty() {
        segments.push(StyledSegment {
            text: text.to_owned(),
            style,
        });
    }
}

fn clipped_label(label: &str, width: usize) -> String {
    if visual_width(label) <= width {
        label.to_owned()
    } else if width == 0 {
        String::new()
    } else {
        let prefix = &label[..split_at_cells(label, width - 1)];
        let prefix = if visual_width(prefix) < width {
            prefix
        } else {
            ""
        };
        format!("{prefix}…")
    }
}

fn summary_footer(width: usize, rows: usize, failed: bool) -> Vec<StyledSegment> {
    let count_width = if rows == 0 {
        0
    } else {
        rows.to_string()
            .len()
            .max(5)
            .min(width.saturating_sub(if failed { 8 } else { 2 }))
    };
    let mut segments = Vec::new();
    if failed {
        let label = if width >= 7 { " failed" } else { "failed" };
        push_segment(
            &mut segments,
            &clipped_label(label, width),
            Style::default()
                .fg(COLOR_DIFF_REMOVE)
                .add_modifier(Modifier::BOLD),
        );
    }
    if count_width > 0 {
        let count = clipped_label(&rows.to_string(), count_width);
        push_segment(
            &mut segments,
            &format!(" {count:>count_width$}"),
            Style::default().fg(COLOR_STEP8),
        );
    }
    segments
}

fn summary_arguments(message: &Message, width: usize) -> Vec<StyledSegment> {
    let label = tool_arguments(tool_label(message));
    let counts = (tool_gutter(message).0 == "Δ")
        .then(|| split_diff_counts(label))
        .flatten();
    let mut segments = Vec::new();
    let style = Style::default().fg(COLOR_TOOL_TEXT);
    if let Some((path, added, removed)) =
        counts.filter(|(_, a, r)| a.len() + visual_width(r) < width)
    {
        push_segment(
            &mut segments,
            &clipped_label(path, width - added.len() - visual_width(&removed)),
            style,
        );
        push_segment(&mut segments, &added, Style::default().fg(COLOR_DIFF_ADD));
        push_segment(
            &mut segments,
            &removed,
            Style::default().fg(COLOR_DIFF_REMOVE),
        );
    } else {
        push_segment(&mut segments, &clipped_label(label, width), style);
    }
    segments
}

/// Keep compact rows on one line even when the command contains a long script.
pub(super) fn summary_line(message: &Message, width: usize, rows: usize) -> RenderedLine {
    let footer = summary_footer(width, rows, tool_failed(message));
    let footer_width: usize = footer.iter().map(|s| visual_width(&s.text)).sum();
    let available = width.saturating_sub(footer_width);
    let mut segments = summary_arguments(message, available);
    if footer_width > 0 {
        let label_width: usize = segments.iter().map(|s| visual_width(&s.text)).sum();
        push_segment(
            &mut segments,
            &" ".repeat(available.saturating_sub(label_width)),
            Style::default(),
        );
    }
    segments.extend(footer);
    let text: String = segments.iter().map(|s| s.text.as_str()).collect();
    RenderedLine {
        cells: visual_width(&text),
        text,
        styled_segments: segments,
        role: Role::ToolCall,
        separator: false,
        soft_wrap_to_next: false,
    }
}

/// Wrap AI descriptions completely while keeping literal command labels on one row.
pub(super) fn summary_lines(message: &Message, width: usize, rows: usize) -> Vec<RenderedLine> {
    if width == 0 {
        return Vec::new();
    }
    if !message
        .tool_output
        .as_ref()
        .is_some_and(|tool| tool.generated)
    {
        return vec![summary_line(message, width, rows)];
    }
    let footer = summary_footer(width, rows, tool_failed(message));
    let footer_width: usize = footer
        .iter()
        .map(|segment| visual_width(&segment.text))
        .sum();
    let mut lines: Vec<_> = wrap_natural_by_cells(tool_arguments(tool_label(message)), width)
        .into_iter()
        .map(ai_label_line)
        .collect();
    if lines
        .last()
        .is_some_and(|line| line.cells + footer_width > width)
    {
        lines.push(ai_label_line(String::new()));
    }
    if let Some(last) = lines.last_mut() {
        if footer_width > 0 {
            let padding = " ".repeat(width.saturating_sub(last.cells + footer_width));
            push_segment(&mut last.styled_segments, &padding, Style::default());
            last.styled_segments.extend(footer);
            last.text = last
                .styled_segments
                .iter()
                .map(|segment| segment.text.as_str())
                .collect();
            last.cells = visual_width(&last.text);
        }
    }
    let count = lines.len();
    for (index, line) in lines.iter_mut().enumerate() {
        line.soft_wrap_to_next = index + 1 < count;
    }
    lines
}

fn ai_label_line(text: String) -> RenderedLine {
    let cells = visual_width(&text);
    RenderedLine {
        styled_segments: vec![StyledSegment {
            text: text.clone(),
            style: Style::default().fg(COLOR_TOOL_TEXT),
        }],
        text,
        cells,
        role: Role::ToolCall,
        separator: false,
        soft_wrap_to_next: false,
    }
}
