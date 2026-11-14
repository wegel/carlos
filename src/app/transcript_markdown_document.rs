//! Measure one whole parsed Markdown document while keeping only requested painted rows.

use std::borrow::Cow;
use std::mem::size_of;

use carlos_core::session::models::Role;

use crate::app::text::wrap_natural_by_cells;
use crate::app::transcript_styles::{
    append_wrapped_styled_logical_lines, markdown_line_segments, styled_plain_text,
};
use crate::app::view_models::{RenderedLine, StyledSegment};

/// Keep the existing whole-document parser's logical lines and a compact wrapped-row index.
pub(super) struct DocumentFrame {
    lines: Vec<Vec<StyledSegment>>,
    starts: Vec<usize>,
    width: usize,
}

impl DocumentFrame {
    /// Parse the complete source so late references, tables and nested constructs retain meaning.
    pub fn parse(text: &str, width: usize) -> Self {
        Self {
            lines: markdown_line_segments(text, width),
            starts: Vec::new(),
            width,
        }
    }

    /// Count actual old-formatter wraps without retaining their row strings.
    pub fn index(&mut self) {
        let mut starts = Vec::with_capacity(self.lines.len() + 1);
        starts.push(0);
        let (mut previous, mut count) = (String::new(), 0);
        for line in &self.lines {
            let text = if let [span] = line.as_slice() {
                Cow::Borrowed(span.text.as_str())
            } else {
                Cow::Owned(styled_plain_text(line))
            };
            let rows = if count > 0 && text.as_ref() == previous {
                count
            } else {
                wrap_natural_by_cells(&text, self.width).len()
            };
            if text.len() <= 4096 {
                previous.clear();
                previous.push_str(&text);
                count = rows;
            }
            starts.push(starts.last().copied().unwrap_or(0) + rows);
        }
        self.starts = starts;
    }

    /// Count all indexed wrapped rows.
    pub fn rows(&self) -> usize {
        self.starts.last().copied().unwrap_or(0)
    }

    /// Count retained logical-line storage separately from its row index.
    pub fn document_capacity_bytes(&self) -> usize {
        self.lines.capacity() * size_of::<Vec<StyledSegment>>()
            + self
                .lines
                .iter()
                .map(|line| {
                    line.capacity() * size_of::<StyledSegment>()
                        + line.iter().map(|span| span.text.capacity()).sum::<usize>()
                })
                .sum::<usize>()
    }

    /// Count the single compact prefix-sum allocation.
    pub fn index_capacity_bytes(&self) -> usize {
        self.starts.capacity() * size_of::<usize>()
    }

    /// Reuse the old painter for touched logical lines, retaining only the requested rows.
    /// A very long physical line still needs a saved within-line index before production use.
    pub fn window(&self, start: usize, length: usize) -> Vec<RenderedLine> {
        let end = start.saturating_add(length).min(self.rows());
        if start >= end {
            return Vec::new();
        }
        let first = self
            .starts
            .partition_point(|row| *row <= start)
            .saturating_sub(1);
        let mut result = Vec::new();
        for index in first..self.lines.len() {
            let offset = self.starts[index];
            if offset >= end {
                break;
            }
            let mut painted = Vec::new();
            append_wrapped_styled_logical_lines(
                &mut painted,
                Role::Assistant,
                vec![self.lines[index].clone()],
                self.width,
            );
            result.extend(
                painted
                    .into_iter()
                    .skip(start.saturating_sub(offset))
                    .take(end.saturating_sub(offset.max(start))),
            );
        }
        assert_eq!(result.len(), end - start);
        result
    }
}

#[cfg(test)]
#[path = "transcript_markdown_document_tests.rs"]
mod tests;
