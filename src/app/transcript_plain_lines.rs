//! Visit exact plain-message logical lines and preserve the existing fence rules.

use std::ops::Range;

use carlos_core::session::models::Role;
use ratatui::style::Style;

use crate::app::text::visual_width;
use crate::app::transcript_styles::is_fence_delimiter;
use crate::app::view_models::{RenderedLine, StyledSegment};
use crate::theme::COLOR_ROW_USER;

/// Distinguish real blank lines from wrapped text and user code-fence text.
#[derive(Clone, Copy)]
pub(super) enum LineKind {
    Blank,
    Text(bool),
}

/// Visit logical lines in one batch and return the fence state after its final byte.
pub(super) fn visit_lines(
    source: &str,
    original: Range<usize>,
    last: bool,
    role: Role,
    mut code: bool,
    mut visit: impl FnMut(Range<usize>, LineKind),
) -> bool {
    let raw = &source[original.clone()];
    let text = if last {
        raw
    } else {
        raw.strip_suffix('\n').unwrap_or(raw)
    };
    let mut byte = original.start;
    for line in text.split('\n') {
        let range = byte..byte + line.len();
        byte += line.len() + 1;
        if is_fence_delimiter(line) {
            if matches!(role, Role::User) {
                visit(range, LineKind::Blank);
            }
            code = !code;
        } else if line.is_empty() {
            visit(range, LineKind::Blank);
        } else {
            visit(range, LineKind::Text(code && matches!(role, Role::User)));
        }
    }
    code
}

/// Paint a plain row with the existing empty-row and user-fence styles.
pub(super) fn paint(text: String, kind: LineKind, role: Role, wrapped: bool) -> RenderedLine {
    let styled_segments = match kind {
        LineKind::Blank => Vec::new(),
        LineKind::Text(code) => vec![StyledSegment {
            text: text.clone(),
            style: if code {
                Style::default().bg(COLOR_ROW_USER)
            } else {
                Style::default()
            },
        }],
    };
    RenderedLine {
        cells: visual_width(&text),
        text,
        role,
        separator: false,
        styled_segments,
        soft_wrap_to_next: wrapped,
    }
}
