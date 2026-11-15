//! Measure one whole parsed Markdown document while keeping only requested painted rows.

#[path = "transcript_styled_line.rs"]
mod styled_line;

use std::borrow::Cow;
#[cfg(test)]
use std::mem::size_of;

use carlos_core::session::models::Role;

use self::styled_line::StyledLine;
use crate::app::text::wrap_natural_by_cells;
use crate::app::transcript_styles::{append_wrapped_styled_logical_lines, styled_plain_text};
use crate::app::transcript_wrap::markdown_logical_lines;
use crate::app::view_models::{RenderedLine, StyledSegment};

enum LineStyle {
    ConsumedSpans,
    Uniform,
}

/// Keep the existing whole-document parser's logical lines and a compact wrapped-row index.
pub(super) struct DocumentFrame {
    lines: Vec<Vec<StyledSegment>>,
    starts: Vec<usize>,
    width: usize,
    role: Role,
    long: Vec<(usize, StyledLine)>,
    style: LineStyle,
}

impl DocumentFrame {
    /// Parse the complete source so late references, tables and nested constructs retain meaning.
    #[cfg(test)]
    pub fn parse(text: &str, width: usize) -> Self {
        Self::parse_role(Role::Assistant, text, width)
    }

    /// Preserve the existing reasoning styles and whole-document assistant parser.
    pub fn parse_role(role: Role, text: &str, width: usize) -> Self {
        Self::from_lines(markdown_logical_lines(role, text, width), role, width)
    }

    /// Retain logical lines from an existing parser without copying their strings.
    pub fn from_lines(lines: Vec<Vec<StyledSegment>>, role: Role, width: usize) -> Self {
        Self {
            lines,
            starts: Vec::new(),
            width,
            role,
            long: Vec::new(),
            style: LineStyle::ConsumedSpans,
        }
    }

    /// Preserve the fallback diff painter's uniform tint even when wraps skip spaces.
    pub fn uniform_lines(lines: Vec<Vec<StyledSegment>>, role: Role, width: usize) -> Self {
        debug_assert!(lines.iter().all(|line| line.len() <= 1));
        let mut document = Self::from_lines(lines, role, width);
        document.style = LineStyle::Uniform;
        document
    }

    /// Count actual old-formatter wraps without retaining their row strings.
    pub fn index(&mut self) {
        let mut starts = Vec::with_capacity(self.lines.len() + 1);
        starts.push(0);
        self.long.clear();
        let (mut previous, mut count) = (String::new(), 0);
        for (index, line) in self.lines.iter().enumerate() {
            if line.iter().map(|span| span.text.len()).sum::<usize>() > 16 * 1024 {
                let saved = StyledLine::new(line, self.width);
                starts.push(starts.last().copied().unwrap_or(0) + saved.rows());
                self.long.push((index, saved));
                continue;
            }
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
    #[cfg(test)]
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

    /// Count the row prefix sums and saved ranges within long logical lines.
    #[cfg(test)]
    pub fn index_capacity_bytes(&self) -> usize {
        self.starts.capacity() * size_of::<usize>()
            + self.long.capacity() * size_of::<(usize, StyledLine)>()
            + self
                .long
                .iter()
                .map(|(_, line)| line.capacity_bytes())
                .sum::<usize>()
    }

    /// Revisit saved long-line ranges and paint only bounded ordinary logical lines.
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
            let mut rows = self.paint_line(
                index,
                start.saturating_sub(offset),
                end.saturating_sub(offset.max(start)),
            );
            if matches!(self.style, LineStyle::Uniform) {
                self.tint_line(index, &mut rows);
            }
            result.extend(rows);
        }
        assert_eq!(result.len(), end - start);
        result
    }

    fn paint_line(&self, index: usize, first: usize, length: usize) -> Vec<RenderedLine> {
        if let Ok(saved) = self.long.binary_search_by_key(&index, |(line, _)| *line) {
            return self.long[saved]
                .1
                .window(&self.lines[index], self.role, first, length);
        }
        let mut painted = Vec::new();
        append_wrapped_styled_logical_lines(
            &mut painted,
            self.role,
            vec![self.lines[index].clone()],
            self.width,
        );
        painted.into_iter().skip(first).take(length).collect()
    }

    fn tint_line(&self, index: usize, rows: &mut [RenderedLine]) {
        let Some(style) = self.lines[index].first().map(|span| span.style) else {
            return;
        };
        for row in rows.iter_mut().filter(|row| !row.text.is_empty()) {
            row.styled_segments = vec![StyledSegment {
                text: row.text.clone(),
                style,
            }];
        }
    }
}

#[cfg(test)]
#[path = "transcript_markdown_document_tests.rs"]
mod tests;
