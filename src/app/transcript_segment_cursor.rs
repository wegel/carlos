//! Consume styled spans by borrowed byte offsets, without copying their unconsumed suffixes.

use super::text_window::unicode_cut;
use crate::app::text::visual_width;
use crate::app::view_models::StyledSegment;

/// Keep a read position inside immutable styled spans.
pub(super) struct SegmentCursor<'a> {
    spans: &'a [StyledSegment],
    index: usize,
    byte: usize,
    last_non_ascii: Option<usize>,
    last_tab: Option<usize>,
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
        }
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
            let whole_cells = if simple {
                text.len()
            } else {
                visual_width(text)
            };
            let partial = whole_cells > allowed;
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
            self.byte += cut;
            if self.byte == span.text.len() {
                self.index += 1;
                self.byte = 0;
                self.last_non_ascii = last_non_ascii(self.spans.get(self.index));
                self.last_tab = last_tab(self.spans.get(self.index));
            }
            if partial || taken == cells {
                break;
            }
        }
        result
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
