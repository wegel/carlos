//! Page plain-message rows with exact fence rules and compact long-line ranges.

#[path = "transcript_plain_lines.rs"]
mod lines;
#[path = "transcript_plain_long_line.rs"]
mod long_line;

#[cfg(test)]
use std::mem::size_of;
use std::sync::Arc;

use carlos_core::session::models::Role;

use self::lines::{paint, visit_lines, LineKind};
use self::long_line::LongLine;
use super::text_window::visit_rows;
use super::PageError;
use crate::app::view_models::RenderedLine;

const BATCH_BYTES: usize = 16 * 1024;
const BATCH_LINES: usize = 128;

struct Checkpoint {
    byte: usize,
    row: usize,
    code: bool,
    long: Option<Box<LongLine>>,
}

/// Retain one exact plain source and materialize only requested terminal rows.
pub(in crate::app) struct PlainPages {
    source: Arc<String>,
    role: Role,
    width: usize,
    checkpoints: Vec<Checkpoint>,
    rows: usize,
    #[cfg(test)]
    offset: usize,
}

impl PlainPages {
    /// Index the existing plain-message wrapper's rows at a supported width.
    pub fn new(source: Arc<String>, role: Role, width: usize) -> Result<Self, PageError> {
        Self::from_offset(source, role, width, 0)
    }

    /// Index a UTF-8 suffix while retaining the complete original for copy or another formatter.
    pub fn from_offset(
        source: Arc<String>,
        role: Role,
        width: usize,
        start: usize,
    ) -> Result<Self, PageError> {
        if width < 8 {
            return Err(PageError::WidthTooSmall);
        }
        if source.get(start..).is_none() {
            return Err(PageError::InvalidOffset);
        }
        let mut pages = Self {
            source,
            role,
            width,
            checkpoints: Vec::new(),
            rows: 0,
            #[cfg(test)]
            offset: start,
        };
        pages.index_from(start, 0, false);
        Ok(pages)
    }

    /// Revisit the last unfinished batch and append new original bytes.
    pub fn append(&mut self, text: &str) {
        let last = self
            .checkpoints
            .pop()
            .expect("even an empty source has one batch");
        let (byte, row, code) = (last.byte, last.row, last.code);
        drop(last);
        Arc::make_mut(&mut self.source).push_str(text);
        self.index_from(byte, row, code);
    }

    /// Recount the same source at a new width without copying the original.
    #[cfg(test)]
    pub fn resize(mut self, width: usize) -> Result<Self, PageError> {
        if width < 8 {
            return Err(PageError::WidthTooSmall);
        }
        self.width = width;
        self.checkpoints.clear();
        self.index_from(self.offset, 0, false);
        Ok(self)
    }

    /// Borrow the unchanged original for the formatter owner's clipboard path.
    pub fn original_text(&self) -> &str {
        &self.source
    }

    /// Consume this index and move its original into another formatter model.
    pub fn into_original(self) -> Arc<String> {
        self.source
    }

    /// Return the exact wrapped-row count, including a final empty logical line.
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Charge every retained checkpoint, source run and wrapped-row range.
    #[cfg(test)]
    pub fn index_capacity_bytes(&self) -> usize {
        self.checkpoints.capacity() * size_of::<Checkpoint>()
            + self
                .checkpoints
                .iter()
                .filter_map(|point| point.long.as_ref())
                .map(|long| long.capacity_bytes())
                .sum::<usize>()
    }

    /// Paint only batches intersecting the requested terminal rows.
    pub fn window(&self, start: usize, length: usize) -> Vec<RenderedLine> {
        let end = start.saturating_add(length).min(self.rows);
        if start >= end {
            return Vec::new();
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
            let from = start.saturating_sub(point.row);
            let length = end.saturating_sub(point.row + from);
            if let Some(long) = &point.long {
                result.extend(long.window(&self.source, self.role, from, length));
            } else {
                let until = self
                    .checkpoints
                    .get(index + 1)
                    .map_or(self.source.len(), |next| next.byte);
                result.extend(self.paint_batch(point.byte, until, point.code, from, length));
            }
        }
        debug_assert_eq!(result.len(), end - start);
        result
    }

    fn index_from(&mut self, mut byte: usize, mut rows: usize, mut code: bool) {
        loop {
            let end = batch_end(&self.source, byte);
            let long = (end - byte > BATCH_BYTES).then(|| {
                Box::new(LongLine::new(
                    &self.source,
                    byte..end,
                    self.role,
                    self.width,
                    code,
                ))
            });
            let (count, after) = long.as_ref().map_or_else(
                || self.count_batch(byte, end, code),
                |long| (long.rows(), long.after_code),
            );
            self.checkpoints.push(Checkpoint {
                byte,
                row: rows,
                code,
                long,
            });
            rows += count;
            code = after;
            byte = end;
            if byte == self.source.len() {
                break;
            }
        }
        self.rows = rows;
    }

    fn count_batch(&self, byte: usize, end: usize, code: bool) -> (usize, bool) {
        let mut rows = 0;
        let after = visit_lines(
            &self.source,
            byte..end,
            end == self.source.len(),
            self.role,
            code,
            |bytes, kind| {
                rows += if matches!(kind, LineKind::Blank) {
                    1
                } else {
                    visit_rows(&self.source[bytes], self.width, |_, _| {})
                };
            },
        );
        (rows, after)
    }

    fn paint_batch(
        &self,
        byte: usize,
        end: usize,
        code: bool,
        start: usize,
        length: usize,
    ) -> Vec<RenderedLine> {
        let mut result = Vec::new();
        let mut offset = 0;
        let wanted_end = start.saturating_add(length);
        visit_lines(
            &self.source,
            byte..end,
            end == self.source.len(),
            self.role,
            code,
            |bytes, kind| {
                if matches!(kind, LineKind::Blank) {
                    if offset >= start && offset < wanted_end {
                        result.push(paint(String::new(), kind, self.role, false));
                    }
                    offset += 1;
                    return;
                }
                let rows = visit_rows(&self.source[bytes], self.width, |row, part| {
                    if offset + row >= start && offset + row < wanted_end {
                        result.push(paint(part.to_string(), kind, self.role, true));
                    }
                });
                offset += rows;
                if offset > start && offset <= wanted_end {
                    if let Some(last) = result.last_mut() {
                        last.soft_wrap_to_next = false;
                    }
                }
            },
        );
        result
    }
}

fn batch_end(source: &str, start: usize) -> usize {
    let limit = start.saturating_add(BATCH_BYTES).min(source.len());
    let mut end = start;
    let mut lines = 0;
    for byte in start..limit {
        if source.as_bytes()[byte] == b'\n' {
            end = byte + 1;
            lines += 1;
            if lines == BATCH_LINES {
                return end;
            }
        }
    }
    if limit == source.len() {
        source.len()
    } else if end > start {
        end
    } else {
        source.as_bytes()[limit..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(source.len(), |next| limit + next + 1)
    }
}

#[cfg(test)]
#[path = "transcript_plain_pages_tests.rs"]
mod tests;
