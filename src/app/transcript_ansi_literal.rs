//! Index one contiguous styled ASCII word by byte arithmetic instead of revisiting its prefix.

use std::ops::Range;

use carlos_core::session::models::Role;
use ratatui::style::Style;

use super::seed::Batch;
use crate::app::view_models::{RenderedLine, StyledSegment};

/// Revisit one styled word directly in its original contiguous source range.
pub(super) struct LiteralLine {
    pub bytes: Range<usize>,
    style: Style,
}

impl LiteralLine {
    /// Recognize a single styled word whose bytes remain contiguous in the original source.
    pub fn from_batch(batch: &Batch, raw: &str, offset: usize) -> Option<Self> {
        let [line] = batch.lines.as_slice() else {
            return None;
        };
        let [span] = line.as_slice() else {
            return None;
        };
        if span.text.is_empty() || !Self::can_append(&span.text) {
            return None;
        }
        let [bytes] = batch.ranges.first()?.as_slice() else {
            return None;
        };
        if raw.get(bytes.clone()) != Some(span.text.as_str()) {
            return None;
        }
        let start = bytes.start + offset;
        Some(Self {
            bytes: start..start + span.text.len(),
            style: span.style,
        })
    }

    /// Alphanumeric ASCII has no natural word-break opportunity or grapheme boundary to revisit.
    pub fn can_append(text: &str) -> bool {
        text.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }

    /// Count the word's exact hard wraps using its ASCII byte length.
    pub fn rows(&self, width: usize) -> usize {
        self.bytes.len().div_ceil(width)
    }

    /// Paint the requested rows without reading the word's preceding bytes.
    pub fn window(
        &self,
        source: &str,
        role: Role,
        width: usize,
        start: usize,
        length: usize,
    ) -> Vec<RenderedLine> {
        let rows = self.rows(width);
        let end = start.saturating_add(length).min(rows);
        (start..end)
            .map(|row| {
                let from = self.bytes.start + row * width;
                let until = from.saturating_add(width).min(self.bytes.end);
                let text = &source[from..until];
                RenderedLine {
                    text: text.to_string(),
                    cells: text.len(),
                    role,
                    separator: false,
                    styled_segments: vec![StyledSegment {
                        text: text.to_string(),
                        style: self.style,
                    }],
                    soft_wrap_to_next: row + 1 < rows,
                }
            })
            .collect()
    }
}
