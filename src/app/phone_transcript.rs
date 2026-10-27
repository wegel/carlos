//! Separate phone entries with shared headings or tool symbols on their own rows.

use ratatui::style::{Color, Modifier, Style};

use super::models::{Message, MessageKind, RenderedLine, Role, StyledSegment};
use super::text::{split_at_cells, visual_width};
use super::tool_output_state::compact_tool_message;
use super::tool_summary::{summary_line_count, summary_lines, tool_gutter};
use super::transcript_render::tool_header_rows;
use super::transcript_spacing::{
    inline_entry, should_insert_separator_between, single_line_reasoning,
};
use crate::theme::{role_gutter_fg, role_gutter_symbol};

#[derive(PartialEq, Eq)]
struct Section {
    glyph: &'static str,
    label: &'static str,
    color: Color,
}

fn section(message: &Message) -> Section {
    let tool = matches!(message.role, Role::ToolCall | Role::ToolOutput);
    let (glyph, color) = if tool {
        tool_gutter(message)
    } else {
        (
            role_gutter_symbol(message.role),
            role_gutter_fg(message.role),
        )
    };
    let label = match message.role {
        Role::User => "you",
        Role::Assistant => "answer",
        Role::Reasoning => "reasoning",
        Role::Commentary => "commentary",
        Role::System => "system",
        Role::ToolCall | Role::ToolOutput => match glyph {
            "→" => "read",
            "✱" => "search",
            "Δ" => "diff/edit",
            _ => "run",
        },
    };
    Section {
        glyph,
        label,
        color,
    }
}

fn inline_tool(message: &Message) -> bool {
    matches!(message.role, Role::ToolCall | Role::ToolOutput)
}

/// Keep short reasoning beside its symbol only when it fits the current viewport.
pub(super) fn inline_reasoning(message: &Message, width: usize) -> bool {
    single_line_reasoning(message, width.saturating_sub(2))
}

/// Share one heading across adjacent entries of the same type.
fn needs_header(previous: Option<&Message>, message: &Message, width: usize) -> bool {
    let current = section(message);
    !inline_entry(message, width.saturating_sub(2))
        && previous.is_none_or(|prev| {
            let previous = section(prev);
            inline_entry(prev, width.saturating_sub(2))
                || previous.glyph != current.glyph
                || previous.label != current.label
        })
}

fn leading_rows(previous: Option<&Message>, message: &Message, width: usize) -> (bool, bool) {
    let heading = width >= 8 && needs_header(previous, message, width);
    let blank = previous.is_some_and(|previous| {
        should_insert_separator_between(previous, message, width.saturating_sub(2))
    });
    (blank, heading)
}

fn blank_line() -> RenderedLine {
    RenderedLine {
        cells: 0,
        text: String::new(),
        styled_segments: Vec::new(),
        role: Role::System,
        separator: true,
        soft_wrap_to_next: false,
    }
}

fn existing_label_rows(message: &Message, width: usize) -> usize {
    match message.kind {
        MessageKind::ToolSummary { rows } => summary_line_count(message, width, rows),
        _ if message.role == Role::ToolCall => summary_line_count(message, width, 0),
        _ => tool_header_rows(message, width),
    }
}

/// Count the same entry rows that the phone view will build, including blank lines.
pub(super) fn count_rows(
    previous: Option<&Message>,
    original: &Message,
    display: &Message,
    width: usize,
    desktop_count: usize,
) -> usize {
    let blank = previous.is_some_and(|prev| should_insert_separator_between(prev, display, width));
    let body = desktop_count.saturating_sub(usize::from(blank));
    if body == 0 {
        return 0;
    }
    let body = if width >= 8 && inline_tool(original) {
        body.saturating_sub(existing_label_rows(display, width))
            + inline_tool_labels(original, display, width).len()
    } else {
        body
    };
    let (blank, heading) = leading_rows(previous, original, width);
    body + usize::from(blank) + usize::from(heading)
}

/// Keep code at its full width and put each tool's symbol beside its label.
pub(super) fn format_block(
    block: &mut Vec<RenderedLine>,
    previous: Option<&Message>,
    original: &Message,
    display: &Message,
    width: usize,
) {
    if block.first().is_some_and(|line| line.separator) {
        block.remove(0);
    }
    if block.is_empty() {
        return;
    }
    if inline_reasoning(original, width) {
        prepend_symbol(&mut block[0], "◇", role_gutter_fg(Role::Reasoning));
    }
    if width >= 8 && inline_tool(original) {
        let labels = inline_tool_labels(original, display, width);
        block.splice(..existing_label_rows(display, width), labels);
    }
    let (blank, heading) = leading_rows(previous, original, width);
    if heading {
        block.insert(0, header(original, width));
    }
    if blank {
        block.insert(0, blank_line());
    }
}

fn inline_tool_labels(original: &Message, display: &Message, width: usize) -> Vec<RenderedLine> {
    let rows = match display.kind {
        MessageKind::ToolSummary { rows } => rows,
        _ => 0,
    };
    let summary = compact_tool_message(display, rows);
    let mut labels = summary_lines(summary.as_ref().unwrap_or(display), width - 2, rows);
    let (glyph, color) = tool_gutter(original);
    for (index, label) in labels.iter_mut().enumerate() {
        prepend_symbol(label, if index == 0 { glyph } else { " " }, color);
    }
    labels
}

fn prepend_symbol(line: &mut RenderedLine, glyph: &str, color: Color) {
    let prefix = format!("{glyph} ");
    line.text.insert_str(0, &prefix);
    line.cells += 2;
    line.styled_segments.insert(
        0,
        StyledSegment {
            text: prefix,
            style: Style::default().fg(color),
        },
    );
}

/// Keep headers out of copied text by marking them as separator rows.
pub(super) fn header(message: &Message, width: usize) -> RenderedLine {
    let Section {
        glyph,
        label,
        color,
        ..
    } = section(message);
    let title = format!("─ {glyph} {label} ");
    let title = &title[..split_at_cells(&title, width)];
    let text = format!(
        "{title}{}",
        "─".repeat(width.saturating_sub(visual_width(title)))
    );
    RenderedLine {
        cells: visual_width(&text),
        text: text.clone(),
        styled_segments: vec![StyledSegment {
            text,
            style: Style::default().fg(color).add_modifier(Modifier::BOLD),
        }],
        role: message.role,
        separator: true,
        soft_wrap_to_next: false,
    }
}
