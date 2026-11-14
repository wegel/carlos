//! Prototype source/style checkpoints that paint a bounded batch instead of a whole ANSI body.

#[path = "transcript_ansi_batch.rs"]
mod batch;
#[path = "transcript_ansi_literal.rs"]
mod literal;
#[path = "transcript_ansi_long_line.rs"]
mod long_line;
#[path = "transcript_ansi_seed.rs"]
mod seed;
#[path = "transcript_segment_cursor.rs"]
mod segment_cursor;
#[path = "transcript_source_ranges.rs"]
mod source_ranges;
#[path = "transcript_text_window.rs"]
mod text_window;

#[path = "transcript_plain_pages.rs"]
pub(super) mod plain;

use std::mem::size_of;
use std::sync::Arc;

use carlos_core::session::models::Role;
use ratatui::style::Style;

use self::batch::{batch_end, paint_window, LastCount};
use self::literal::LiteralLine;
use self::long_line::LongLine;
pub(in crate::app) use self::segment_cursor::SegmentCursor;
pub(in crate::app) use self::source_ranges::SourceRanges;
pub(in crate::app) use self::text_window::visit_ranges;
use crate::app::view_models::RenderedLine;

const BATCH_BYTES: usize = 16 * 1024;
const BATCH_LINES: usize = 128;

#[derive(Debug, PartialEq, Eq)]
/// Explain why this private ANSI prototype cannot construct or revisit an index.
pub(super) enum PageError {
    NotAnsi,
    WidthTooSmall,
    InvalidAnsi,
    InvalidOffset,
}

struct Checkpoint {
    byte: usize,
    row: usize,
    style: Style,
    literal: Option<LiteralLine>,
    long: Option<Box<LongLine>>,
}

/// Save style and source offsets while retaining only painted viewport rows.
pub(super) struct AnsiPages {
    source: Arc<String>,
    role: Role,
    width: usize,
    checkpoints: Vec<Checkpoint>,
    rows: usize,
}

impl AnsiPages {
    /// Index one original ANSI source at the complete-body formatter's width.
    pub fn new(source: Arc<String>, role: Role, width: usize) -> Result<Self, PageError> {
        if width < 8 {
            return Err(PageError::WidthTooSmall);
        }
        if !source.contains(['\x1b', '\u{009b}']) {
            return Err(PageError::NotAnsi);
        }
        let mut pages = Self {
            source,
            role,
            width,
            checkpoints: Vec::new(),
            rows: 0,
        };
        pages.index_from(0, 0, Style::default())?;
        Ok(pages)
    }

    /// Recount the last unfinished batch and the newly appended original bytes only.
    pub fn append(&mut self, text: &str) -> Result<(), PageError> {
        if self.append_literal(text) {
            return Ok(());
        }
        let from = self.checkpoints.pop().unwrap_or(Checkpoint {
            byte: 0,
            row: 0,
            style: Style::default(),
            literal: None,
            long: None,
        });
        let (byte, row, style) = (from.byte, from.row, from.style);
        drop(from);
        Arc::make_mut(&mut self.source).push_str(text);
        self.index_from(byte, row, style)
    }

    /// Revisit the same original at a new width without copying the retained source.
    pub fn resize(mut self, width: usize) -> Result<Self, PageError> {
        if width < 8 {
            return Err(PageError::WidthTooSmall);
        }
        self.width = width;
        let mut rows = 0;
        let mut cache = LastCount::default();
        for index in 0..self.checkpoints.len() {
            let until = self
                .checkpoints
                .get(index + 1)
                .map_or(self.source.len(), |point| point.byte);
            let point = &mut self.checkpoints[index];
            point.row = rows;
            if let Some(literal) = &point.literal {
                rows += literal.rows(width);
            } else {
                point.long = None;
                let parse = if until - point.byte > BATCH_BYTES {
                    seed::parse_mapped
                } else {
                    seed::parse
                };
                let batch = parse(&self.source[point.byte..until], point.style)
                    .ok_or(PageError::InvalidAnsi)?;
                point.long = (until - point.byte > BATCH_BYTES)
                    .then(|| Box::new(LongLine::new(&batch, &self.source, point.byte, width)));
                rows += point.long.as_ref().map_or_else(
                    || {
                        batch
                            .lines
                            .iter()
                            .map(|line| cache.count(line, width))
                            .sum()
                    },
                    |long| long.rows(),
                );
            }
        }
        self.rows = rows;
        Ok(self)
    }

    fn append_literal(&mut self, text: &str) -> bool {
        let Some(point) = self.checkpoints.last_mut() else {
            return false;
        };
        let Some(literal) = &mut point.literal else {
            return false;
        };
        if literal.bytes.end != self.source.len() || !LiteralLine::can_append(text) {
            return false;
        }
        Arc::make_mut(&mut self.source).push_str(text);
        literal.bytes.end += text.len();
        self.rows = point.row + literal.rows(self.width);
        true
    }

    fn index_from(
        &mut self,
        mut byte: usize,
        mut rows: usize,
        mut style: Style,
    ) -> Result<(), PageError> {
        let mut cache = LastCount::default();
        while byte < self.source.len() {
            let end = batch_end(&self.source, byte);
            let parse = if end - byte > BATCH_BYTES {
                seed::parse_mapped
            } else {
                seed::parse
            };
            let batch = parse(&self.source[byte..end], style).ok_or(PageError::InvalidAnsi)?;
            let literal = (end - byte > BATCH_BYTES)
                .then(|| LiteralLine::from_batch(&batch, &self.source[byte..end], byte))
                .flatten();
            let long = (end - byte > BATCH_BYTES && literal.is_none())
                .then(|| Box::new(LongLine::new(&batch, &self.source, byte, self.width)));
            let count = if let Some(literal) = &literal {
                literal.rows(self.width)
            } else if let Some(long) = &long {
                long.rows()
            } else {
                batch
                    .lines
                    .iter()
                    .map(|line| cache.count(line, self.width))
                    .sum()
            };
            self.checkpoints.push(Checkpoint {
                byte,
                row: rows,
                style,
                literal,
                long,
            });
            rows += count;
            style = batch.after;
            byte = end;
        }
        self.rows = rows;
        Ok(())
    }

    /// Count all wrapped rows without retaining their painted strings.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Measure retained checkpoint storage separately from the original source.
    pub fn index_capacity_bytes(&self) -> usize {
        self.checkpoints.capacity() * size_of::<Checkpoint>()
            + self
                .checkpoints
                .iter()
                .filter_map(|point| point.long.as_ref())
                .map(|long| long.capacity_bytes())
                .sum::<usize>()
    }

    /// Borrow the exact original without sharing its ownership outside the formatter.
    pub fn original_text(&self) -> &str {
        &self.source
    }

    /// Consume this index and move its original into another formatter model.
    pub fn into_original(self) -> Arc<String> {
        self.source
    }

    /// Reparse only checkpoints intersecting the requested wrapped rows.
    pub fn window(&self, start: usize, length: usize) -> Result<Vec<RenderedLine>, PageError> {
        let end = start.saturating_add(length).min(self.rows);
        if start >= end {
            return Ok(Vec::new());
        }
        let first = self
            .checkpoints
            .partition_point(|point| point.row <= start)
            .saturating_sub(1);
        let mut result = Vec::new();
        for index in first..self.checkpoints.len() {
            let point = &self.checkpoints[index];
            if point.row >= end {
                break;
            }
            result.extend(self.paint_checkpoint(index, start, end)?);
        }
        debug_assert_eq!(result.len(), end - start);
        Ok(result)
    }

    fn paint_checkpoint(
        &self,
        index: usize,
        start: usize,
        end: usize,
    ) -> Result<Vec<RenderedLine>, PageError> {
        let point = &self.checkpoints[index];
        let start = start.saturating_sub(point.row);
        let length = end.saturating_sub(point.row + start);
        if let Some(literal) = &point.literal {
            return Ok(literal.window(&self.source, self.role, self.width, start, length));
        }
        if let Some(long) = &point.long {
            return Ok(long.window(&self.source, self.role, start, length));
        }
        let until = self
            .checkpoints
            .get(index + 1)
            .map_or(self.source.len(), |next| next.byte);
        let batch = seed::parse(&self.source[point.byte..until], point.style)
            .ok_or(PageError::InvalidAnsi)?;
        Ok(paint_window(
            &batch.lines,
            self.role,
            self.width,
            start,
            length,
        ))
    }
}

#[cfg(test)]
#[path = "transcript_ansi_pages_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "transcript_ansi_long_probe.rs"]
mod long_probe;
