use textwrap::{wrap as wrap_text, Options as WrapOptions, WordSplitter};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) fn visual_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

pub(super) fn split_at_cells(s: &str, max_cells: usize) -> usize {
    if max_cells == 0 || s.is_empty() {
        return 0;
    }

    let mut cells = 0usize;
    let mut idx = 0usize;

    for (byte_idx, g) in s.grapheme_indices(true) {
        let w = visual_width(g);
        if w > 0 && cells + w > max_cells {
            break;
        }
        cells += w;
        idx = byte_idx + g.len();
    }

    if idx == 0 {
        if let Some(g) = s.graphemes(true).next() {
            return g.len();
        }
    }

    idx
}

pub(super) fn slice_by_cells(s: &str, start: usize, end: usize) -> String {
    if start >= end || s.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    let mut col = 0usize;

    for g in s.graphemes(true) {
        let w = visual_width(g);
        if w == 0 {
            if col >= start && col < end {
                out.push_str(g);
            }
            continue;
        }

        let next = col + w;
        if next <= start {
            col = next;
            continue;
        }
        if col >= end {
            break;
        }

        out.push_str(g);
        col = next;
    }
    out
}

pub(super) fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    if char_idx == 0 {
        return 0;
    }
    s.char_indices()
        .nth(char_idx)
        .map(|(idx, _)| idx)
        .unwrap_or(s.len())
}

pub(super) fn wrap_natural_by_cells(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    if text.is_empty() {
        return vec![String::new()];
    }
    if text.is_ascii() && text.len() <= width {
        return vec![text.to_string()];
    }
    if visual_width(text) <= width {
        return vec![text.to_string()];
    }

    let options = WrapOptions::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);
    let wrapped = wrap_text(text, options);
    if wrapped.is_empty() {
        return vec![String::new()];
    }

    let mut out = Vec::new();
    for piece in wrapped {
        let s = piece.into_owned();
        if visual_width(&s) <= width {
            out.push(s);
            continue;
        }

        // Extremely long tokens can still overflow when word breaking is disabled.
        // Fall back to hard cell wrapping only in that case.
        let mut rest = s.as_str();
        loop {
            let take = split_at_cells(rest, width);
            if take == 0 {
                out.push(rest.to_string());
                break;
            }
            out.push(rest[..take].to_string());
            if take >= rest.len() {
                break;
            }
            rest = &rest[take..];
        }
    }
    preserve_trailing_spaces(&mut out, text, width);
    out
}

pub(super) fn wrap_natural_count_by_cells(text: &str, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    if text.is_empty() {
        return 1;
    }
    if text.is_ascii() && text.len() <= width {
        return 1;
    }
    if visual_width(text) <= width {
        return 1;
    }

    wrap_natural_count_slow_by_cells(text, width)
}

pub(super) fn count_ascii_multiline_by_cells(text: &str, width: usize) -> Option<usize> {
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
        count += if line.is_empty() {
            1
        } else if line.len() <= width {
            1
        } else {
            wrap_natural_count_slow_by_cells(line, width)
        };
        start = idx.saturating_add(1);
    }

    Some(count)
}

fn wrap_natural_count_slow_by_cells(text: &str, width: usize) -> usize {
    debug_assert!(width > 0);
    debug_assert!(!text.is_empty());

    let options = WrapOptions::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);
    let wrapped = wrap_text(text, options);
    if wrapped.is_empty() {
        return 1;
    }

    let mut line_count = 0usize;
    let mut last_line_width = 0usize;
    let mut last_trailing_spaces = 0usize;

    for piece in wrapped {
        let piece = piece.as_ref();
        if visual_width(piece) <= width {
            line_count += 1;
            last_line_width = visual_width(piece);
            last_trailing_spaces = piece.chars().rev().take_while(|c| *c == ' ').count();
            continue;
        }

        let mut rest = piece;
        loop {
            let take = split_at_cells(rest, width);
            if take == 0 {
                line_count += 1;
                last_line_width = visual_width(rest);
                last_trailing_spaces = rest.chars().rev().take_while(|c| *c == ' ').count();
                break;
            }
            let part = &rest[..take];
            line_count += 1;
            last_line_width = visual_width(part);
            last_trailing_spaces = part.chars().rev().take_while(|c| *c == ' ').count();
            if take >= rest.len() {
                break;
            }
            rest = &rest[take..];
        }
    }

    additional_lines_for_trailing_spaces(text, width, last_line_width, last_trailing_spaces)
        + line_count
}

pub(super) fn wrap_input_line(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    if line.is_empty() {
        return vec![String::new()];
    }

    let mut out = wrap_natural_by_cells(line, width);
    if out.is_empty() {
        out.push(String::new());
    }

    preserve_trailing_spaces(&mut out, line, width);
    out
}

#[cfg(test)]
pub(super) fn wrap_input_line_count(line: &str, width: usize) -> usize {
    wrap_natural_count_by_cells(line, width)
}

fn preserve_trailing_spaces(out: &mut Vec<String>, text: &str, width: usize) {
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

fn additional_lines_for_trailing_spaces(
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
