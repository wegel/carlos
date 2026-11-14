//! Save wrapped row and style-consumption ranges for oversized ANSI physical lines.

use std::mem::size_of;
use std::ops::Range;

use carlos_core::session::models::Role;
use ratatui::style::Style;

use super::seed::Batch;
use super::{visit_ranges, SegmentCursor, SourceRanges};
use crate::app::text::visual_width;
use crate::app::transcript_styles::{normalize_styled_segments_for_part, styled_plain_text};
use crate::app::view_models::{RenderedLine, StyledSegment};

struct Span {
    end: usize,
    original: usize,
    style: Style,
}

struct Row {
    text: Range<usize>,
    consumed: Range<usize>,
    wrapped: bool,
}

/// Retain byte ranges rather than parsed or painted strings for one oversized batch.
pub(super) struct LongLine {
    text: SourceRanges,
    spans: Vec<Span>,
    rows: Vec<Row>,
}

impl LongLine {
    /// Keep the upstream parser's exact original spans and complete-line word breaks.
    pub fn new(batch: &Batch, source: &str, offset: usize, width: usize) -> Self {
        let mut index = Self {
            text: SourceRanges::default(),
            spans: Vec::new(),
            rows: Vec::new(),
        };
        let mut plain_offset = 0;
        for (line, ranges) in batch.lines.iter().zip(&batch.ranges) {
            let text_offset = index.text.bytes();
            let originals: Vec<_> = ranges
                .iter()
                .map(|bytes| offset + bytes.start..offset + bytes.end)
                .collect();
            let expanded = {
                let plain = styled_plain_text(line);
                index.text.append_line(&originals, source, &plain)
            };
            for (span, bytes) in line.iter().zip(ranges) {
                let original = offset + bytes.start..offset + bytes.end;
                plain_offset += span.text.len();
                index.spans.push(Span {
                    end: plain_offset,
                    original: original.start,
                    style: span.style,
                });
            }
            index.index_rows(line, &expanded, text_offset, plain_offset, width);
        }
        index.text.shrink_to_fit();
        index.spans.shrink_to_fit();
        index.rows.shrink_to_fit();
        index
    }

    /// Count the saved rows without reparsing the original line.
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// Paint saved rows directly, without walking their preceding text or styles.
    pub fn window(
        &self,
        source: &str,
        role: Role,
        start: usize,
        length: usize,
    ) -> Vec<RenderedLine> {
        let end = start.saturating_add(length).min(self.rows.len());
        self.rows[start.min(end)..end]
            .iter()
            .map(|row| {
                let text = self.text.text(source, row.text.clone());
                let styled = self.styled(source, row.consumed.clone());
                RenderedLine {
                    cells: visual_width(&text),
                    styled_segments: normalize_styled_segments_for_part(&text, styled),
                    text,
                    role,
                    separator: false,
                    soft_wrap_to_next: row.wrapped,
                }
            })
            .collect()
    }

    /// Count every retained range and span slot separately from the source bytes.
    pub fn capacity_bytes(&self) -> usize {
        size_of::<Self>()
            + self.text.capacity_bytes()
            + self.spans.capacity() * size_of::<Span>()
            + self.rows.capacity() * size_of::<Row>()
    }

    fn index_rows(
        &mut self,
        line: &[StyledSegment],
        expanded: &str,
        text_offset: usize,
        plain_end: usize,
        width: usize,
    ) {
        let plain_offset = plain_end - line.iter().map(|span| span.text.len()).sum::<usize>();
        let mut cursor = SegmentCursor::new(line);
        visit_ranges(expanded, width, |_, bytes, part| {
            let consumed = cursor.position();
            cursor.take(visual_width(part), false);
            self.rows.push(Row {
                text: text_offset + bytes.start..text_offset + bytes.end,
                consumed: plain_offset + consumed..plain_offset + cursor.position(),
                wrapped: true,
            });
        });
        if let Some(last) = self.rows.last_mut() {
            last.wrapped = false;
        }
    }

    fn styled(&self, source: &str, wanted: Range<usize>) -> Vec<StyledSegment> {
        let mut result = Vec::new();
        let first = self.spans.partition_point(|span| span.end <= wanted.start);
        for index in first..self.spans.len() {
            let span = &self.spans[index];
            let start = index
                .checked_sub(1)
                .map_or(0, |before| self.spans[before].end);
            if start >= wanted.end {
                break;
            }
            let from = wanted.start.saturating_sub(start);
            let until = wanted.end.min(span.end) - start;
            result.push(StyledSegment {
                text: source[span.original + from..span.original + until].to_string(),
                style: span.style,
            });
        }
        result
    }
}
