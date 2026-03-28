use textwrap::word_splitters::split_words;
use textwrap::{wrap as wrap_text, Options as WrapOptions, WordSplitter};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub(super) fn visual_width(s: &str) -> usize {
    if s.is_ascii() {
        return s.len();
    }
    UnicodeWidthStr::width(s)
}

pub(super) fn split_at_cells(s: &str, max_cells: usize) -> usize {
    if max_cells == 0 || s.is_empty() {
        return 0;
    }
    if s.is_ascii() {
        return s.len().min(max_cells);
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

fn wrap_natural_count_slow_by_cells(text: &str, width: usize) -> usize {
    debug_assert!(width > 0);
    debug_assert!(!text.is_empty());

    if text.is_ascii() {
        return wrap_natural_count_ascii_slow_by_cells(text, width);
    }

    let options = WrapOptions::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);

    let words = options.word_separator.find_words(text);
    let broken_words = split_words(words, &options.word_splitter).collect::<Vec<_>>();
    if broken_words.is_empty() {
        return 1;
    }

    let line_widths = [width];
    let wrapped_words = options.wrap_algorithm.wrap(&broken_words, &line_widths);

    let mut line_count = 0usize;
    let mut last_metrics: Option<(usize, usize)> = None;

    let mut idx = 0usize;
    for words in wrapped_words {
        let Some(last_word) = words.last() else {
            line_count += 1;
            last_metrics = Some((0, 0));
            continue;
        };

        let len = words
            .iter()
            .map(|word| word.len() + word.whitespace.len())
            .sum::<usize>()
            .saturating_sub(last_word.whitespace.len());
        let line = &text[idx..idx + len];
        let (wrapped_count, wrapped_last_width, wrapped_last_spaces) =
            hard_wrap_count_metrics(line, width);
        line_count += wrapped_count;
        last_metrics = Some((wrapped_last_width, wrapped_last_spaces));
        idx += len + last_word.whitespace.len();
    }

    let (last_line_width, last_trailing_spaces) = last_metrics.unwrap_or((0, 0));

    additional_lines_for_trailing_spaces(text, width, last_line_width, last_trailing_spaces)
        + line_count
}

fn wrap_natural_count_ascii_slow_by_cells(text: &str, width: usize) -> usize {
    let options = WrapOptions::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);

    let mut line_count = 0usize;

    let mut current_idx = 0usize;
    let mut line_start = 0usize;
    let mut line_visible_len = 0usize;
    let mut line_visible_width = 0usize;
    let mut pending_whitespace = 0usize;
    let mut have_line = false;

    for word in split_words(
        options.word_separator.find_words(text),
        &options.word_splitter,
    ) {
        let word_len = word.len();
        let whitespace_len = word.whitespace.len();
        let projected_width = if have_line {
            line_visible_width + pending_whitespace + word_len
        } else {
            word_len
        };

        if !have_line || projected_width <= width {
            if !have_line {
                line_start = current_idx;
                line_visible_len = word_len;
                line_visible_width = word_len;
                have_line = true;
            } else {
                line_visible_len += pending_whitespace + word_len;
                line_visible_width = projected_width;
            }
            pending_whitespace = whitespace_len;
        } else {
            let line = &text[line_start..line_start + line_visible_len];
            let (wrapped_count, _, _) = hard_wrap_count_metrics(line, width);
            line_count += wrapped_count;

            line_start = current_idx;
            line_visible_len = word_len;
            line_visible_width = word_len;
            pending_whitespace = whitespace_len;
        }

        current_idx += word_len + whitespace_len;
    }

    if !have_line {
        return 1;
    }

    let line = &text[line_start..line_start + line_visible_len];
    let (wrapped_count, last_line_width, last_trailing_spaces) =
        hard_wrap_count_metrics(line, width);
    line_count += wrapped_count;

    additional_lines_for_trailing_spaces(text, width, last_line_width, last_trailing_spaces)
        + line_count
}

fn hard_wrap_count_metrics(text: &str, width: usize) -> (usize, usize, usize) {
    let line_width = visual_width(text);
    if line_width <= width {
        return (
            1,
            line_width,
            text.chars().rev().take_while(|c| *c == ' ').count(),
        );
    }

    let mut count = 0usize;
    let mut last_line_width;
    let mut last_trailing_spaces;
    let mut rest = text;
    loop {
        let take = split_at_cells(rest, width);
        if take == 0 {
            count += 1;
            last_line_width = visual_width(rest);
            last_trailing_spaces = rest.chars().rev().take_while(|c| *c == ' ').count();
            break;
        }
        let part = &rest[..take];
        count += 1;
        last_line_width = visual_width(part);
        last_trailing_spaces = part.chars().rev().take_while(|c| *c == ' ').count();
        if take >= rest.len() {
            break;
        }
        rest = &rest[take..];
    }

    (count, last_line_width, last_trailing_spaces)
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
