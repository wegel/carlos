//! Choose Flow section headings while sharing body and tool layout with Gutter.

use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use ratatui::style::{Color, Modifier, Style};

use super::layout_state::TranscriptTheme;
use super::text::{split_at_cells, visual_width};
use super::tool_summary::{tool_failed, tool_gutter};
use super::transcript_input::RenderInput;
use super::transcript_spacing::{inline_entry, single_line_reasoning};
use super::view_models::{RenderedLine, StyledSegment};
use crate::theme::{
    role_gutter_fg, role_gutter_symbol, COLOR_DIFF_REMOVE, COLOR_GUTTER_TOOL_CALL, COLOR_TOOL_EDIT,
    COLOR_TOOL_READ, COLOR_TOOL_SEARCH,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SectionKind {
    User,
    Steer,
    Answer,
    Reasoning,
    Commentary,
    System,
    Read,
    Search,
    Edit,
    Run,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Section {
    kind: SectionKind,
    failed: bool,
}

impl Section {
    fn details(self) -> (&'static str, &'static str, Color) {
        let (role, label) = match self.kind {
            SectionKind::User => (Role::User, "you"),
            SectionKind::Steer => (Role::User, "you (steer)"),
            SectionKind::Answer => (Role::Assistant, "answer"),
            SectionKind::Reasoning => (Role::Reasoning, "reasoning"),
            SectionKind::Commentary => (Role::Commentary, "commentary"),
            SectionKind::System => (Role::System, "system"),
            SectionKind::Read => return self.tool("→", "read", COLOR_TOOL_READ),
            SectionKind::Search => return self.tool("✱", "search", COLOR_TOOL_SEARCH),
            SectionKind::Edit => return self.tool("Δ", "diff/edit", COLOR_TOOL_EDIT),
            SectionKind::Run => return self.tool("$", "run", COLOR_GUTTER_TOOL_CALL),
        };
        (role_gutter_symbol(role), label, role_gutter_fg(role))
    }

    fn tool(
        self,
        glyph: &'static str,
        label: &'static str,
        color: Color,
    ) -> (&'static str, &'static str, Color) {
        (
            glyph,
            label,
            if self.failed {
                COLOR_DIFF_REMOVE
            } else {
                color
            },
        )
    }
}

/// Keep heading and spacing facts after an entry's source leaves the body cache.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct EntryContext {
    inline: bool,
    section: Option<Section>,
}

impl EntryContext {
    /// Resolve only the source-dependent facts used by the following entry's heading.
    pub(super) fn new(message: RenderInput<'_>, width: usize, theme: TranscriptTheme) -> Self {
        Self {
            inline: inline_entry(
                message,
                width.saturating_sub(if theme.is_flow() { 2 } else { 0 }),
            ),
            section: theme.is_flow().then(|| section(message)),
        }
    }

    /// Separate entries unless both belong to the inline activity group.
    pub(super) fn separates(self, next: Self) -> bool {
        !(self.inline && next.inline)
    }
}

fn section(message: RenderInput<'_>) -> Section {
    let kind = match message.role {
        Role::User if message.kind == MessageKind::UserSteer => SectionKind::Steer,
        Role::User => SectionKind::User,
        Role::Assistant => SectionKind::Answer,
        Role::Reasoning => SectionKind::Reasoning,
        Role::Commentary => SectionKind::Commentary,
        Role::System => SectionKind::System,
        Role::ToolCall | Role::ToolOutput => match tool_gutter(message).0 {
            "→" => SectionKind::Read,
            "✱" => SectionKind::Search,
            "Δ" => SectionKind::Edit,
            _ => SectionKind::Run,
        },
    };
    Section {
        kind,
        failed: matches!(message.role, Role::ToolCall | Role::ToolOutput) && tool_failed(message),
    }
}

/// Put short reasoning beside its symbol only when the text fits.
pub(super) fn inline_reasoning<'a>(message: impl Into<RenderInput<'a>>, width: usize) -> bool {
    single_line_reasoning(message.into(), width.saturating_sub(2))
}

/// Reuse saved neighbor facts when its original source is no longer resident.
pub(super) fn section_header_from_context(
    previous: Option<EntryContext>,
    current: EntryContext,
    message: RenderInput<'_>,
    width: usize,
) -> Option<RenderedLine> {
    if width < 8 || current.inline {
        return None;
    }
    let shared = message.kind != MessageKind::UserSteer
        && previous.is_some_and(|previous| !previous.inline && previous.section == current.section);
    (!shared).then(|| header(message, width))
}

fn header(message: RenderInput<'_>, width: usize) -> RenderedLine {
    let (glyph, label, color) = section(message).details();
    let title = if message.kind == MessageKind::UserSteer && width < 16 {
        format!("{glyph} steer ")
    } else {
        format!("─ {glyph} {label} ")
    };
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
