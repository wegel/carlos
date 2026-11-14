//! Retain source and row ranges for oversized plain logical lines.

use std::mem::size_of;
use std::ops::Range;

use carlos_core::session::models::Role;

use super::super::source_ranges::SourceRanges;
use super::super::text_window::visit_ranges;
use super::lines::{paint, visit_lines, LineKind};
use crate::app::view_models::RenderedLine;

struct Row {
    text: Range<usize>,
    kind: LineKind,
    wrapped: bool,
}

/// Save a complete long line's optimal word breaks without retaining painted rows.
pub(super) struct LongLine {
    text: SourceRanges,
    rows: Vec<Row>,
    pub after_code: bool,
}

impl LongLine {
    /// Index a batch against its original byte coordinates and initial fence state.
    pub fn new(source: &str, original: Range<usize>, role: Role, width: usize, code: bool) -> Self {
        let mut index = Self {
            text: SourceRanges::default(),
            rows: Vec::new(),
            after_code: code,
        };
        let last = original.end == source.len();
        index.after_code = visit_lines(source, original, last, role, code, |bytes, kind| {
            if matches!(kind, LineKind::Blank) {
                index.rows.push(Row {
                    text: 0..0,
                    kind,
                    wrapped: false,
                });
                return;
            }
            let offset = index.text.bytes();
            let plain = &source[bytes.clone()];
            let expanded = index.text.append_line(&[bytes], source, plain);
            visit_ranges(&expanded, width, |_, range, _| {
                index.rows.push(Row {
                    text: offset + range.start..offset + range.end,
                    kind,
                    wrapped: true,
                });
            });
            if let Some(row) = index.rows.last_mut() {
                row.wrapped = false;
            }
        });
        index.text.shrink_to_fit();
        index.rows.shrink_to_fit();
        index
    }

    /// Count saved rows without walking their preceding text.
    pub fn rows(&self) -> usize {
        self.rows.len()
    }

    /// Paint only requested saved rows and their exact fence backgrounds.
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
                paint(
                    self.text.text(source, row.text.clone()),
                    row.kind,
                    role,
                    row.wrapped,
                )
            })
            .collect()
    }

    /// Charge all retained source-run and row slots to the formatter index.
    pub fn capacity_bytes(&self) -> usize {
        size_of::<Self>() + self.text.capacity_bytes() + self.rows.capacity() * size_of::<Row>()
    }
}
