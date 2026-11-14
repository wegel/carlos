//! Visit naturally wrapped rows without retaining a string for every row of a long line.

#[path = "transcript_optimal_fragments.rs"]
mod optimal;

use std::ops::Range;

use textwrap::{wrap, Options, WordSplitter};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::text::{expand_tabs, visual_width};

/// Locate wrapped rows within a caller's already tab-expanded complete logical line.
pub(in crate::app) fn visit_ranges(
    text: &str,
    width: usize,
    mut visit: impl FnMut(usize, Range<usize>, &str),
) -> usize {
    debug_assert!(!text.contains('\t'));
    let mut byte = 0;
    visit_rows(text, width, |row, part| {
        let from = byte
            + text[byte..]
                .find(part)
                .expect("wrapped row borrows logical text");
        byte = from + part.len();
        visit(row, from..byte, part);
    })
}

/// Use the same complete-line word wrapping as the terminal's existing formatter.
pub(in crate::app) fn visit_rows(
    text: &str,
    width: usize,
    visit: impl FnMut(usize, &str),
) -> usize {
    visit_rows_inner(text, width, visit, true)
}

/// Keep the previous public wrap path available for isolated allocation diagnostics.
#[cfg(test)]
pub(in crate::app) fn visit_legacy_rows(
    text: &str,
    width: usize,
    visit: impl FnMut(usize, &str),
) -> usize {
    visit_rows_inner(text, width, visit, false)
}

/// Measure the private compact record without exposing its fields to formatter callers.
#[cfg(test)]
pub(in crate::app) fn compact_word_bytes() -> usize {
    std::mem::size_of::<optimal::CompactWord>()
}

/// Measure the exceptional compact record without exposing its fields.
#[cfg(test)]
pub(in crate::app) fn wide_word_bytes() -> usize {
    std::mem::size_of::<optimal::WideWord>()
}

fn visit_rows_inner(
    text: &str,
    width: usize,
    visit: impl FnMut(usize, &str),
    compact: bool,
) -> usize {
    if width == 0 {
        return 0;
    }
    let expanded;
    let text = if text.contains('\t') {
        expanded = expand_tabs(text);
        expanded.as_str()
    } else {
        text
    };
    let mut emit = Rows {
        visit,
        count: 0,
        width,
        trailing: text.bytes().rev().take_while(|byte| *byte == b' ').count(),
    };
    if text.is_empty() || visual_width(text) <= width {
        emit.part(text, true);
        return emit.count;
    }
    let options = Options::new(width)
        .break_words(false)
        .word_splitter(WordSplitter::NoHyphenation);
    let used =
        compact && optimal::visit_pieces(text, &options, |piece, last| emit.piece(piece, last));
    if !used {
        let pieces = wrap(text, options);
        for (index, piece) in pieces.iter().enumerate() {
            emit.piece(piece, index + 1 == pieces.len());
        }
    }
    if emit.count == 0 {
        emit.part("", true);
    }
    emit.count
}

/// Apply the old grapheme rule after the caller has ruled out an ASCII suffix.
pub(in crate::app) fn unicode_cut(text: &str, width: usize) -> usize {
    if width > 0
        && text.len() > width
        && text.as_bytes()[..=width]
            .iter()
            .all(|byte| byte.is_ascii() && !matches!(byte, b'\t' | b'\r' | b'\n'))
    {
        // The next ASCII scalar proves the preceding byte is a full grapheme.
        return width;
    }
    let (mut cells, mut end) = (0, 0);
    for (byte, grapheme) in text.grapheme_indices(true) {
        let size = visual_width(grapheme);
        if size > 0 && cells + size > width {
            break;
        }
        cells += size;
        end = byte + grapheme.len();
    }
    if end == 0 {
        text.graphemes(true).next().map_or(0, str::len)
    } else {
        end
    }
}

struct Rows<F> {
    visit: F,
    count: usize,
    width: usize,
    trailing: usize,
}

impl<F: FnMut(usize, &str)> Rows<F> {
    fn piece(&mut self, piece: &str, last: bool) {
        if visual_width(piece) <= self.width {
            self.part(piece, last);
            return;
        }
        let ascii = piece.is_ascii();
        let mut rest = piece;
        while !rest.is_empty() {
            let cut = if ascii {
                rest.len().min(self.width)
            } else {
                unicode_cut(rest, self.width)
            };
            let cut = if cut == 0 { rest.len() } else { cut };
            self.part(&rest[..cut], last && cut == rest.len());
            rest = &rest[cut..];
        }
    }

    fn part(&mut self, part: &str, last: bool) {
        let present = part.bytes().rev().take_while(|byte| *byte == b' ').count();
        let missing = if last {
            self.trailing.saturating_sub(present)
        } else {
            0
        };
        if missing == 0 {
            self.emit(part);
            return;
        }
        let add = missing.min(self.width.saturating_sub(visual_width(part)));
        let mut tail = part.to_string();
        tail.extend(std::iter::repeat_n(' ', add));
        self.emit(&tail);
        let mut missing = missing - add;
        while missing > 0 {
            let add = missing.min(self.width);
            self.emit(&" ".repeat(add));
            missing -= add;
        }
    }

    fn emit(&mut self, part: &str) {
        (self.visit)(self.count, part);
        self.count += 1;
    }
}

#[cfg(test)]
#[path = "transcript_text_window_tests.rs"]
mod tests;
