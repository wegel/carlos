//! Trailing-space preservation, tab expansion, and overflow-row counting for wrapped text.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{visual_width, TAB_STOP};

pub(super) fn preserve_trailing_spaces(out: &mut Vec<String>, text: &str, width: usize) {
    // Preserve trailing spaces so transcript copy/paste and input editing
    // can round-trip explicit whitespace at the end of a logical line.
    let wanted_trailing_spaces = text.chars().rev().take_while(|c| *c == ' ').count();
    if wanted_trailing_spaces == 0 {
        return;
    }

    let mut present_trailing_spaces = out
        .last()
        .map(|s| s.chars().rev().take_while(|c| *c == ' ').count())
        .unwrap_or(0);
    let mut missing = wanted_trailing_spaces.saturating_sub(present_trailing_spaces);
    while missing > 0 {
        let last_idx = out.len().saturating_sub(1);
        let last_width = visual_width(&out[last_idx]);
        let avail = width.saturating_sub(last_width);
        if avail == 0 {
            out.push(String::new());
            continue;
        }
        let add = missing.min(avail);
        out[last_idx].push_str(&" ".repeat(add));
        present_trailing_spaces += add;
        missing = wanted_trailing_spaces.saturating_sub(present_trailing_spaces);
        if missing > 0 && add == avail {
            out.push(String::new());
        }
    }
}

pub(super) fn additional_lines_for_trailing_spaces(
    text: &str,
    width: usize,
    last_line_width: usize,
    present_trailing_spaces: usize,
) -> usize {
    let wanted_trailing_spaces = text.chars().rev().take_while(|c| *c == ' ').count();
    let missing = wanted_trailing_spaces.saturating_sub(present_trailing_spaces);
    if missing == 0 || width == 0 {
        return 0;
    }

    let available_on_last_line = width.saturating_sub(last_line_width);
    if missing <= available_on_last_line {
        return 0;
    }

    let remaining = missing - available_on_last_line;
    remaining.div_ceil(width)
}

// --- Tab Expansion ---

pub(super) fn expand_tabs(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut col = 0usize;
    for grapheme in text.graphemes(true) {
        if grapheme == "\n" || grapheme == "\r" {
            out.push_str(grapheme);
            col = 0;
        } else if grapheme == "\t" {
            let spaces = TAB_STOP - (col % TAB_STOP);
            out.push_str(&" ".repeat(spaces));
            col += spaces;
        } else {
            out.push_str(grapheme);
            col += UnicodeWidthStr::width(grapheme);
        }
    }
    out
}
