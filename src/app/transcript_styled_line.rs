//! Save exact wraps and style cursor positions for a long retained Markdown, source or diff line.

#[cfg(test)]
use std::mem::size_of;
use std::ops::Range;

use carlos_core::session::models::Role;

use crate::app::text::visual_width;
use crate::app::transcript_body::ansi_pages::{visit_ranges, SegmentCursor, SourceRanges};
use crate::app::transcript_styles::{normalize_styled_segments_for_part, styled_plain_text};
use crate::app::view_models::{RenderedLine, StyledSegment};

struct Row {
    text: Range<usize>,
    consumed: Range<usize>,
}

/// Borrow retained parsed spans at paint time rather than keeping a second flattened string.
pub(super) struct StyledLine {
    text: SourceRanges,
    ends: Vec<usize>,
    rows: Vec<Row>,
}

impl StyledLine {
    /// Save the same whole-line breaks and style consumption used by the existing painter.
    pub fn new(spans: &[StyledSegment], width: usize) -> Self {
        let mut text = SourceRanges::default();
        let mut end = 0;
        let originals: Vec<_> = spans
            .iter()
            .map(|span| {
                let start = end;
                end += span.text.len();
                start..end
            })
            .collect();
        let plain = styled_plain_text(spans);
        let expanded = text.append_line(&originals, &plain, &plain);
        let mut cursor = SegmentCursor::new(spans);
        let mut rows = Vec::new();
        visit_ranges(&expanded, width, |_, bytes, part| {
            let start = cursor.position();
            cursor.take(visual_width(part), false);
            rows.push(Row {
                text: bytes,
                consumed: start..cursor.position(),
            });
        });
        rows.shrink_to_fit();
        text.shrink_to_fit();
        Self {
            text,
            ends: originals.into_iter().map(|range| range.end).collect(),
            rows,
        }
    }

    /// Count saved wrapped rows independently of their strings.
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// Paint only requested rows directly from the retained parsed spans.
    pub fn window(
        &self,
        spans: &[StyledSegment],
        role: Role,
        first: usize,
        length: usize,
    ) -> Vec<RenderedLine> {
        let last = first.saturating_add(length).min(self.rows.len());
        (first.min(last)..last)
            .map(|index| {
                let row = &self.rows[index];
                let text = self.text.text_with(row.text.clone(), |range, out| {
                    self.visit(spans, range, |span, bytes| out.push_str(&span.text[bytes]));
                });
                let mut styled = Vec::new();
                self.visit(spans, row.consumed.clone(), |span, bytes| {
                    styled.push(StyledSegment {
                        text: span.text[bytes].into(),
                        style: span.style,
                    });
                });
                RenderedLine {
                    cells: visual_width(&text),
                    styled_segments: normalize_styled_segments_for_part(&text, styled),
                    text,
                    role,
                    separator: false,
                    soft_wrap_to_next: index + 1 < self.rows.len(),
                }
            })
            .collect()
    }

    /// Include spare row/span/map slots when measuring retained index storage.
    #[cfg(test)]
    pub fn capacity_bytes(&self) -> usize {
        self.text.capacity_bytes()
            + self.ends.capacity() * size_of::<usize>()
            + self.rows.capacity() * size_of::<Row>()
    }

    fn visit(
        &self,
        spans: &[StyledSegment],
        wanted: Range<usize>,
        mut copy: impl FnMut(&StyledSegment, Range<usize>),
    ) {
        let first = self.ends.partition_point(|end| *end <= wanted.start);
        for (index, span) in spans.iter().enumerate().skip(first) {
            let start = index.checked_sub(1).map_or(0, |before| self.ends[before]);
            if start >= wanted.end {
                break;
            }
            copy(
                span,
                wanted.start.saturating_sub(start)..wanted.end.min(self.ends[index]) - start,
            );
        }
    }
}
