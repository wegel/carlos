//! Consume styled spans by borrowed byte offsets, without copying their unconsumed suffixes.

use super::text_window::unicode_cut;
use crate::app::text::visual_width;
use crate::app::view_models::StyledSegment;

/// Keep a read position inside immutable styled spans.
pub(in crate::app) struct SegmentCursor<'a> {
    spans: &'a [StyledSegment],
    index: usize,
    byte: usize,
    last_non_ascii: Option<usize>,
    last_tab: Option<usize>,
    lower_cells: usize,
    consumed: usize,
    #[cfg(test)]
    width_scanned: usize,
}

impl<'a> SegmentCursor<'a> {
    /// Begin at the first span without cloning its text.
    pub fn new(spans: &'a [StyledSegment]) -> Self {
        Self {
            spans,
            index: 0,
            byte: 0,
            last_non_ascii: last_non_ascii(spans.first()),
            last_tab: last_tab(spans.first()),
            lower_cells: 0,
            consumed: 0,
            #[cfg(test)]
            width_scanned: 0,
        }
    }

    /// Locate the consumed prefix in the concatenated, unexpanded span text.
    pub fn position(&self) -> usize {
        self.consumed
    }

    /// Count bytes passed to whole-suffix width measurements in diagnostic tests.
    #[cfg(test)]
    pub fn width_scan_bytes(&self) -> usize {
        self.width_scanned
    }

    /// Skip invisible rows with the same cell-consumption rule as a painted row.
    pub fn take(&mut self, cells: usize, retain: bool) -> Vec<StyledSegment> {
        let mut result = Vec::new();
        let mut taken = 0;
        if cells == 0 {
            return result;
        }
        while let Some(span) = self.spans.get(self.index) {
            let text = &span.text[self.byte..];
            let ascii = self.last_non_ascii.is_none_or(|last| self.byte > last);
            let allowed = cells.saturating_sub(taken);
            let simple = ascii && self.last_tab.is_none_or(|last| self.byte > last);
            #[cfg(test)]
            if !simple && self.lower_cells <= allowed {
                self.width_scanned += text.len();
            }
            let partial = exceeds_allowance(text, allowed, simple, &mut self.lower_cells);
            let cut = if !partial {
                text.len()
            } else if ascii {
                text.len().min(allowed)
            } else {
                unicode_cut(text, allowed)
            };
            if retain && (cut > 0 || text.is_empty()) {
                result.push(StyledSegment {
                    text: text[..cut].to_string(),
                    style: span.style,
                });
            }
            taken += visual_width(&text[..cut]);
            let charge = if simple {
                0
            } else {
                let tabs_after = self.last_tab.is_some_and(|last| last >= self.byte + cut);
                width_charge(&text[..cut], tabs_after)
            };
            self.advance(cut, charge, span.text.len());
            if partial || taken == cells {
                break;
            }
        }
        result
    }

    fn advance(&mut self, bytes: usize, charge: usize, span_bytes: usize) {
        // Keep a lower bound, never assume Unicode widths add across the cut.
        self.lower_cells = self.lower_cells.saturating_sub(charge);
        self.consumed += bytes;
        self.byte += bytes;
        if self.byte == span_bytes {
            self.index += 1;
            self.byte = 0;
            self.last_non_ascii = last_non_ascii(self.spans.get(self.index));
            self.last_tab = last_tab(self.spans.get(self.index));
            self.lower_cells = 0;
        }
    }
}

fn width_charge(prefix: &str, tabs_after: bool) -> usize {
    if prefix.is_empty() {
        return 0;
    }
    let cells = prefix.chars().fold(0_usize, |cells, ch| {
        // ASCII letters/punctuation cannot join an emoji or Unicode ligature.
        // unicode-width 0.1's reverse fold adds at most three cells per scalar.
        // A literal tab can add eight cells when expand_tabs runs first.
        let ascii = ch.is_ascii() && !matches!(ch, '\t' | '#' | '*' | '0'..='9');
        cells.saturating_add(if ascii {
            1
        } else if ch == '\t' {
            8
        } else {
            3
        })
    });
    // Removing a prefix can shift the suffix's first tab by at most seven cells.
    cells.saturating_add(if tabs_after { 7 } else { 0 })
}

fn exceeds_allowance(text: &str, allowed: usize, simple: bool, lower: &mut usize) -> bool {
    if simple {
        text.len() > allowed
    } else if *lower > allowed {
        true
    } else {
        *lower = visual_width(text);
        *lower > allowed
    }
}

fn last_non_ascii(span: Option<&StyledSegment>) -> Option<usize> {
    span.and_then(|span| span.text.bytes().rposition(|byte| !byte.is_ascii()))
}

fn last_tab(span: Option<&StyledSegment>) -> Option<usize> {
    span.and_then(|span| span.text.bytes().rposition(|byte| byte == b'\t'))
}

#[cfg(test)]
#[path = "transcript_segment_cursor_tests.rs"]
mod tests;
