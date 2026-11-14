//! Prototype source/style checkpoints that paint a bounded batch instead of a whole ANSI body.

#[path = "transcript_ansi_literal.rs"]
mod literal;
#[path = "transcript_ansi_seed.rs"]
mod seed;
#[path = "transcript_segment_cursor.rs"]
mod segment_cursor;
#[path = "transcript_text_window.rs"]
mod text_window;

use std::mem::size_of;
use std::sync::Arc;

use carlos_core::session::models::Role;
use ratatui::style::Style;

use self::literal::LiteralLine;
use self::segment_cursor::SegmentCursor;
use self::text_window::visit_rows;
use crate::app::text::visual_width;
use crate::app::transcript_styles::{normalize_styled_segments_for_part, styled_plain_text};
use crate::app::view_models::{RenderedLine, StyledSegment};

const BATCH_BYTES: usize = 16 * 1024;
const BATCH_LINES: usize = 128;

#[derive(Debug, PartialEq, Eq)]
/// Explain why this private ANSI prototype cannot construct or revisit an index.
pub(super) enum PageError {
    NotAnsi,
    WidthTooSmall,
    InvalidAnsi,
}

struct Checkpoint {
    byte: usize,
    row: usize,
    style: Style,
    literal: Option<LiteralLine>,
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
        });
        Arc::make_mut(&mut self.source).push_str(text);
        self.index_from(from.byte, from.row, from.style)
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
                let batch = seed::parse(&self.source[point.byte..until], point.style)
                    .ok_or(PageError::InvalidAnsi)?;
                rows += batch
                    .lines
                    .iter()
                    .map(|line| cache.count(line, width))
                    .sum::<usize>();
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
            let batch =
                seed::parse(&self.source[byte..end], style).ok_or(PageError::InvalidAnsi)?;
            let literal = (end - byte > BATCH_BYTES)
                .then(|| LiteralLine::from_batch(&batch, &self.source[byte..end], byte))
                .flatten();
            let count = literal.as_ref().map_or_else(
                || {
                    batch
                        .lines
                        .iter()
                        .map(|line| cache.count(line, self.width))
                        .sum()
                },
                |literal| literal.rows(self.width),
            );
            self.checkpoints.push(Checkpoint {
                byte,
                row: rows,
                style,
                literal,
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
            if let Some(literal) = &point.literal {
                result.extend(literal.window(
                    &self.source,
                    self.role,
                    self.width,
                    start.saturating_sub(point.row),
                    end.saturating_sub(point.row.max(start)),
                ));
                continue;
            }
            let until = self
                .checkpoints
                .get(index + 1)
                .map_or(self.source.len(), |next| next.byte);
            let batch = seed::parse(&self.source[point.byte..until], point.style)
                .ok_or(PageError::InvalidAnsi)?;
            result.extend(paint_window(
                &batch.lines,
                self.role,
                self.width,
                start.saturating_sub(point.row),
                end.saturating_sub(point.row.max(start)),
            ));
        }
        debug_assert_eq!(result.len(), end - start);
        Ok(result)
    }
}

#[derive(Default)]
struct LastCount {
    text: String,
    rows: usize,
}

impl LastCount {
    fn count(&mut self, line: &[StyledSegment], width: usize) -> usize {
        if line.len() == 1 {
            return self.plain(&line[0].text, width);
        }
        self.plain(&styled_plain_text(line), width)
    }

    fn plain(&mut self, text: &str, width: usize) -> usize {
        if text.is_empty() {
            return 1;
        }
        if self.rows > 0 && text == self.text {
            return self.rows;
        }
        let rows = visit_rows(text, width, |_, _| {});
        if text.len() <= 4096 {
            self.text = text.to_string();
            self.rows = rows;
        }
        rows
    }
}

fn batch_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let limit = start.saturating_add(BATCH_BYTES).min(bytes.len());
    let (mut byte, mut end, mut lines) = (start, start, 0);
    while byte < limit {
        if matches!(bytes[byte], b'\r' | b'\n') {
            byte += if bytes[byte] == b'\r' && bytes.get(byte + 1) == Some(&b'\n') {
                2
            } else {
                1
            };
            end = byte;
            lines += 1;
            if lines == BATCH_LINES {
                return end;
            }
        } else {
            byte += 1;
        }
    }
    if limit == bytes.len() {
        bytes.len()
    } else if end > start {
        end
    } else {
        let next = bytes[limit..]
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
            .map_or(bytes.len(), |next| limit + next);
        if bytes.get(next) == Some(&b'\r') && bytes.get(next + 1) == Some(&b'\n') {
            next + 2
        } else {
            next.saturating_add(1).min(bytes.len())
        }
    }
}

fn paint_window(
    lines: &[Vec<StyledSegment>],
    role: Role,
    width: usize,
    start: usize,
    length: usize,
) -> Vec<RenderedLine> {
    let end = start.saturating_add(length);
    let mut painted = Vec::new();
    let mut offset = 0;
    for line in lines {
        let plain = styled_plain_text(line);
        let mut cursor = SegmentCursor::new(line);
        let rows = visit_rows(&plain, width, |row, part| {
            let at = offset + row;
            if at >= end {
                return;
            }
            let keep = at >= start;
            let cells = visual_width(part);
            let segments = cursor.take(cells, keep);
            if keep {
                painted.push(RenderedLine {
                    text: part.to_string(),
                    cells,
                    role,
                    separator: false,
                    styled_segments: normalize_styled_segments_for_part(part, segments),
                    soft_wrap_to_next: true,
                });
            }
        });
        offset += rows;
        if offset > start && offset <= end {
            if let Some(last) = painted.last_mut() {
                last.soft_wrap_to_next = false;
            }
        }
        if offset >= end {
            break;
        }
    }
    painted
}

#[cfg(test)]
#[path = "transcript_ansi_pages_tests.rs"]
mod tests;
