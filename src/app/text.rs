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
    out
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

    // Preserve trailing spaces immediately so typing "word " visibly updates
    // without waiting for the next non-space character.
    let wanted_trailing_spaces = line.chars().rev().take_while(|c| *c == ' ').count();
    if wanted_trailing_spaces == 0 {
        return out;
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

    out
}
