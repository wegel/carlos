//! Visit naturally wrapped rows without retaining a string for every row of a long line.

use textwrap::{wrap, Options, WordSplitter};
use unicode_segmentation::UnicodeSegmentation;

use crate::app::text::{expand_tabs, visual_width};

/// Use the same complete-line word wrapping as the terminal's existing formatter.
pub(super) fn visit_rows(text: &str, width: usize, visit: impl FnMut(usize, &str)) -> usize {
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
    let pieces = wrap(
        text,
        Options::new(width)
            .break_words(false)
            .word_splitter(WordSplitter::NoHyphenation),
    );
    for (index, piece) in pieces.iter().enumerate() {
        let last = index + 1 == pieces.len();
        if visual_width(piece) <= width {
            emit.part(piece, last);
            continue;
        }
        let ascii = piece.is_ascii();
        let mut rest = piece.as_ref();
        while !rest.is_empty() {
            let cut = if ascii {
                rest.len().min(width)
            } else {
                unicode_cut(rest, width)
            };
            let cut = if cut == 0 { rest.len() } else { cut };
            emit.part(&rest[..cut], last && cut == rest.len());
            rest = &rest[cut..];
        }
    }
    if emit.count == 0 {
        emit.part("", true);
    }
    emit.count
}

/// Apply the old grapheme rule after the caller has ruled out an ASCII suffix.
pub(super) fn unicode_cut(text: &str, width: usize) -> usize {
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
