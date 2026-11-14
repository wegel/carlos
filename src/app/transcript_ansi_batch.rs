//! Count, locate and paint bounded ordinary ANSI batches.

use carlos_core::session::models::Role;

use super::text_window::visit_rows;
use super::{SegmentCursor, BATCH_BYTES, BATCH_LINES};
use crate::app::text::visual_width;
use crate::app::transcript_styles::{normalize_styled_segments_for_part, styled_plain_text};
use crate::app::view_models::{RenderedLine, StyledSegment};

#[derive(Default)]
pub(super) struct LastCount {
    text: String,
    rows: usize,
}

impl LastCount {
    pub(super) fn count(&mut self, line: &[StyledSegment], width: usize) -> usize {
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

pub(super) fn batch_end(source: &str, start: usize) -> usize {
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

pub(super) fn paint_window(
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
