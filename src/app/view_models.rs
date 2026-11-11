//! Borrowed message views and the styled rows drawn in a terminal.

use ratatui::style::Style;

use crate::session::models::{Message, Role};

// --- Message Views ---

/// Choose a full body or a tool label with a terminal-specific hidden-row count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DisplayKind {
    Full,
    ToolSummary { rows: usize },
}

/// Borrow the stored message so a folded view keeps its source and future entry id.
#[derive(Debug, Clone, Copy)]
pub(super) struct DisplayMessage<'a> {
    pub(super) source: &'a Message,
    pub(super) kind: DisplayKind,
}

impl<'a> DisplayMessage<'a> {
    /// Show the stored message without folding its body.
    pub(super) fn full(source: &'a Message) -> Self {
        Self {
            source,
            kind: DisplayKind::Full,
        }
    }

    /// Return the hidden-row count for a folded tool, or zero for a full body.
    pub(super) fn hidden_rows(self) -> usize {
        match self.kind {
            DisplayKind::Full => 0,
            DisplayKind::ToolSummary { rows } => rows,
        }
    }
}

// --- Terminal Rows ---

/// Text with the terminal style applied by the renderer.
#[derive(Debug, Clone)]
pub(super) struct StyledSegment {
    pub(super) text: String,
    pub(super) style: Style,
}

/// One wrapped terminal row and its styled text.
#[derive(Debug, Clone)]
pub(super) struct RenderedLine {
    pub(super) text: String,
    pub(super) styled_segments: Vec<StyledSegment>,
    pub(super) role: Role,
    pub(super) separator: bool,
    pub(super) cells: usize,
    pub(super) soft_wrap_to_next: bool,
}

/// The current terminal width and height in cells.
#[derive(Debug, Clone, Copy)]
pub(super) struct TerminalSize {
    pub(super) width: usize,
    pub(super) height: usize,
}

impl TerminalSize {
    /// Use the phone layout on narrow or short terminals, independently of tool mode.
    pub(super) fn is_small(self) -> bool {
        self.width < 80 || self.height < 20
    }
}
