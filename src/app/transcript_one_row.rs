//! Preserve the legacy reasoning counter while rejecting a second row without counting its suffix.

use carlos_core::session::models::Role;

use crate::app::markdown_tables::table_source;
use crate::app::text::{expand_tabs, split_at_cells, visual_width, wrap_natural_count_by_cells};
use crate::app::transcript_styles::styled_plain_text;
use crate::app::transcript_wrap::{fast_reasoning_summary_plain_text, markdown_logical_lines};
use crate::app::view_models::StyledSegment;

/// Keep the counter's table, strong-summary and empty-parser fallback rules exactly.
pub(super) fn reasoning_uses_one_row(text: &str, width: usize) -> bool {
    if width < 8 {
        return false;
    }
    if table_source(text, width).is_some() {
        return parsed_uses_one_row(markdown_logical_lines(Role::Reasoning, text, width), width)
            .unwrap_or(false);
    }
    let mut one = false;
    for raw in text.split('\n').filter(|line| !line.is_empty()) {
        let result = if let Some(plain) = fast_reasoning_summary_plain_text(raw) {
            Some(plain_uses_one_row(plain, width))
        } else {
            parsed_uses_one_row(markdown_logical_lines(Role::Reasoning, raw, width), width)
        };
        if let Some(fits) = result {
            if one || !fits {
                return false;
            }
            one = true;
        }
    }
    one || parsed_uses_one_row(markdown_logical_lines(Role::Reasoning, text, width), width)
        .unwrap_or(false)
}

/// Match one complete legacy logical-line count, without walking rows after the first cut.
pub(super) fn plain_uses_one_row(text: &str, width: usize) -> bool {
    if width == 0 {
        return false;
    }
    let expanded;
    let text = if text.contains('\t') {
        expanded = expand_tabs(text);
        expanded.as_str()
    } else {
        text
    };
    if visual_width(text) <= width {
        return true;
    }
    if split_at_cells(text, width) < text.len() {
        return false;
    }
    // An overwide grapheme can occupy one old row. Keep the exact optimal-fit
    // decision for such degenerate cases instead of assuming cell widths add.
    wrap_natural_count_by_cells(text, width) == 1
}

fn parsed_uses_one_row(lines: Vec<Vec<StyledSegment>>, width: usize) -> Option<bool> {
    match lines.as_slice() {
        [] => None,
        [line] => Some(plain_uses_one_row(&styled_plain_text(line), width)),
        _ => Some(false),
    }
}
