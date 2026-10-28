//! Choose Flow section headings while sharing body and tool layout with Gutter.

use ratatui::style::{Color, Modifier, Style};

use super::models::{Message, RenderedLine, Role, StyledSegment};
use super::text::{split_at_cells, visual_width};
use super::tool_summary::tool_gutter;
use super::transcript_spacing::{inline_entry, single_line_reasoning};
use crate::theme::{role_gutter_fg, role_gutter_symbol};

#[derive(PartialEq, Eq)]
struct Section {
    glyph: &'static str,
    label: &'static str,
    color: Color,
}

fn section(message: &Message) -> Section {
    let (glyph, color) = if matches!(message.role, Role::ToolCall | Role::ToolOutput) {
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

/// Put short reasoning beside its symbol only when the text fits.
pub(super) fn inline_reasoning(message: &Message, width: usize) -> bool {
    single_line_reasoning(message, width.saturating_sub(2))
}

/// Choose a heading once, before either counting or rendering the block.
pub(super) fn section_header(
    previous: Option<&Message>,
    message: &Message,
    width: usize,
) -> Option<RenderedLine> {
    if width < 8 || inline_entry(message, width.saturating_sub(2)) {
        return None;
    }
    let current = section(message);
    let shared = previous.is_some_and(|previous| {
        !inline_entry(previous, width.saturating_sub(2)) && section(previous) == current
    });
    (!shared).then(|| header(message, width))
}

fn header(message: &Message, width: usize) -> RenderedLine {
    let Section {
        glyph,
        label,
        color,
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
