//! Keep a block's small headings and a painted row interval without allocating its hidden rows.

use std::sync::Arc;

use super::super::view_models::RenderedLine;

/// Address painted rows by their positions in the complete transcript block.
#[derive(Clone)]
pub(in crate::app) struct PaintedRows {
    prefix: Option<Arc<Vec<RenderedLine>>>,
    start: usize,
    lines: Arc<Vec<RenderedLine>>,
}

impl PaintedRows {
    /// Retain an ordinary complete block without copying its strings.
    pub fn whole(lines: Arc<Vec<RenderedLine>>) -> Self {
        Self::window(None, 0, lines)
    }

    /// Retain headings plus only a requested interval of the complete body.
    pub fn window(
        prefix: Option<Arc<Vec<RenderedLine>>>,
        start: usize,
        lines: Arc<Vec<RenderedLine>>,
    ) -> Self {
        Self {
            prefix,
            start,
            lines,
        }
    }

    /// Borrow a row only when its absolute block position is present.
    pub fn get(&self, row: usize) -> Option<&RenderedLine> {
        self.prefix
            .as_ref()
            .and_then(|prefix| prefix.get(row))
            .or_else(|| self.lines.get(row.checked_sub(self.start)?))
    }

    /// Check an inclusive visible interval without walking the rows before it.
    pub fn covers(&self, first: usize, last: usize) -> bool {
        let prefix = self.prefix.as_ref().map_or(0, |prefix| prefix.len());
        first > last
            || last < prefix
            || (first.max(prefix) >= self.start
                && last < self.start.saturating_add(self.lines.len()))
    }

    /// Locate the first body row while small headings remain available.
    pub fn first_content_row(&self) -> usize {
        let mut row = 0;
        while self.get(row).is_some_and(|line| line.separator) {
            row += 1;
        }
        row
    }

    #[cfg(test)]
    pub fn retained(&self) -> impl Iterator<Item = &RenderedLine> {
        self.prefix
            .iter()
            .flat_map(|prefix| prefix.iter())
            .chain(self.lines.iter())
    }
}
