//! Describe entry-local formatter requests after the terminal chooses its visible rows.

use super::RenderCacheState;
use crate::app::flow_transcript::EntryContext;
use crate::app::render_entry_cache::PaintedRows;
use crate::app::render_entry_cache::RowFormat;

/// Freeze row addresses and source identity when the user requests a selection copy.
pub(in crate::app) struct SelectionBlock {
    pub start: usize,
    pub count: usize,
    pub rows: Option<PaintedRows>,
    pub source: Option<carlos_proto::source::Source>,
    pub format: RowFormat,
    pub previous: Option<EntryContext>,
}

impl RenderCacheState {
    /// Capture one block without retaining its complete original or all its painted rows.
    pub(in crate::app) fn selection_block(&self, index: usize) -> Option<SelectionBlock> {
        let row = self.rows.get(index)?;
        let (format, previous, _) = self.format_window(index, row.start)?;
        Some(SelectionBlock {
            start: row.start,
            count: row.count,
            rows: row.lines.clone(),
            source: row.source.clone(),
            format,
            previous,
        })
    }

    /// Describe a visible interval without lending source text to the input thread.
    pub(in crate::app) fn format_window(
        &self,
        index: usize,
        first: usize,
    ) -> Option<(RowFormat, Option<EntryContext>, usize)> {
        let row = self.rows.get(index)?;
        let previous = self.rows[..index]
            .iter()
            .rev()
            .find(|row| row.count > 0)
            .map(|row| row.context);
        Some((
            RowFormat {
                width: self.format.width,
                compact: self.format.compact,
                theme: self.format.theme,
                hidden: self.format.hidden == Some(index),
            },
            previous,
            first.saturating_sub(row.start),
        ))
    }
}
