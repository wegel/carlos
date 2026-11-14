//! Index per-entry layouts by chronological position while source windows load or evict.

use std::sync::Arc;

use carlos_core::session::models::{Message, Role};

use super::flow_transcript::EntryContext;
use super::layout_state::TranscriptTheme;
use super::render_entry_cache::{EntryMemos, PreparedRow, RenderKey, RowFormat, RowSource};
use super::transcript_block::plan_display_block;
use super::transcript_input::RenderInput;
use super::transcript_styles::RenderCountCache;
use super::view_models::RenderedLine;

/// Lend stable row identity, facts and optional source without copying another transcript.
pub(super) trait TranscriptSource {
    fn len(&self) -> usize;
    fn row(&self, index: usize) -> Option<RowSource<'_>>;
    fn input(&self, index: usize) -> Option<RenderInput<'_>> {
        self.row(index)?.input
    }
}

impl TranscriptSource for [Message] {
    fn len(&self) -> usize {
        self.len()
    }
    fn row(&self, index: usize) -> Option<RowSource<'_>> {
        let message = self.get(index)?;
        let input = message.into();
        Some(RowSource {
            key: RenderKey::Entry(message.id),
            generation: None,
            input: Some(input),
            facts: input,
        })
    }
}

struct IndexedRow {
    start: usize,
    count: usize,
    lines: Option<Arc<Vec<RenderedLine>>>,
    context: EntryContext,
    loading: bool,
}

impl IndexedRow {
    fn new(start: usize, row: PreparedRow) -> Self {
        Self {
            start,
            count: row.count,
            lines: row.lines,
            context: row.context,
            loading: row.loading,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ViewFormat {
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
    hidden: Option<usize>,
}

pub(super) struct RenderCacheState {
    rows: Vec<IndexedRow>,
    entries: EntryMemos,
    format: ViewFormat,
    pub(super) rendered_total_lines: usize,
    pub(super) transcript_dirty_from: Option<usize>,
}

impl RenderCacheState {
    /// Keep accepted rows visible but require them to be checked against replacement facts.
    pub(super) fn invalidate_all(&mut self) {
        self.entries.invalidate_all();
        self.transcript_dirty_from = Some(0);
    }

    pub(super) fn new() -> Self {
        Self {
            rows: Vec::new(),
            entries: EntryMemos::default(),
            rendered_total_lines: 0,
            format: ViewFormat {
                width: 0,
                compact: false,
                theme: TranscriptTheme::Gutter,
                hidden: None,
            },
            transcript_dirty_from: Some(0),
        }
    }

    /// Invalidate one changed source, independently of its current list position.
    pub(super) fn invalidate_entry(&mut self, index: usize, key: &RenderKey) {
        let may_have_moved = self
            .transcript_dirty_from
            .is_some_and(|start| start < self.rows.len());
        if index < self.rows.len() || may_have_moved {
            self.entries.invalidate(index, key);
        }
    }

    pub(super) fn mark_transcript_dirty_from(&mut self, messages_len: usize, index: usize) {
        let index = index.min(messages_len);
        self.transcript_dirty_from = Some(
            self.transcript_dirty_from
                .map_or(index, |old| old.min(index)),
        );
    }

    /// Check choices that clear a device's selection before changing its wrapped rows.
    pub(super) fn matches_view(&self, width: usize, compact: bool, theme: TranscriptTheme) -> bool {
        self.format.width == width && self.format.compact == compact && self.format.theme == theme
    }

    /// Keep the embedded caller's suffix invalidation while it still emits positional changes.
    pub(super) fn ensure_rendered_lines(
        &mut self,
        messages: &[Message],
        width: usize,
        hidden: Option<usize>,
        compact: bool,
        theme: TranscriptTheme,
    ) {
        if let Some(start) = self.transcript_dirty_from {
            self.entries.invalidate_suffix(start);
        }
        self.ensure_inputs(messages, width, hidden, compact, theme);
    }

    /// Count changed entries and reuse saved counts for source windows that have evicted.
    pub(super) fn ensure_inputs<S: TranscriptSource + ?Sized>(
        &mut self,
        source: &S,
        width: usize,
        hidden: Option<usize>,
        compact: bool,
        theme: TranscriptTheme,
    ) {
        let format = ViewFormat {
            width,
            compact,
            theme,
            hidden,
        };
        let start = if self.format != format {
            Some(0)
        } else {
            self.transcript_dirty_from
        };
        let Some(start) = start else { return };
        self.format = format;
        self.rebuild(source, start.min(self.rows.len()));
        self.transcript_dirty_from = None;
    }

    fn rebuild<S: TranscriptSource + ?Sized>(&mut self, source: &S, start: usize) {
        // Keep geometric headroom so the first live append does not move the whole cache.
        let capacity = source
            .len()
            .checked_next_power_of_two()
            .unwrap_or(source.len());
        self.entries.reserve(capacity);
        self.rows.reserve(capacity.saturating_sub(self.rows.len()));
        let mut retired = self.entries.begin_suffix(start);
        self.rendered_total_lines = self
            .rows
            .get(start)
            .map_or(self.rendered_total_lines, |row| row.start);
        self.rows.truncate(start);
        if start == 0 {
            self.rendered_total_lines = 0;
        }
        let mut previous = self
            .rows
            .iter()
            .rev()
            .find(|row| row.count > 0)
            .map(|row| row.context);
        let mut cache = RenderCountCache::new();
        for index in start..source.len() {
            let row = source.row(index).expect("current transcript row");
            let format = RowFormat {
                width: self.format.width,
                compact: self.format.compact,
                theme: self.format.theme,
                hidden: self.format.hidden == Some(index) && row.facts.role == Role::User,
            };
            let row = self
                .entries
                .prepare(row, format, previous, &mut cache, &mut retired);
            if row.count > 0 {
                previous = Some(row.context);
            }
            let row = IndexedRow::new(self.rendered_total_lines, row);
            self.rendered_total_lines += row.count;
            self.rows.push(row);
        }
        // Dropping the unused suffix retires entries absent from the current facts.
    }

    /// Count chronological entries without exposing parallel mutable index arrays.
    pub(super) fn entry_count(&self) -> usize {
        self.rows.len()
    }
    /// Locate an entry's first wrapped row.
    pub(super) fn block_start(&self, index: usize) -> Option<usize> {
        Some(self.rows.get(index)?.start)
    }
    /// Count materialized rows for active-step painting.
    pub(super) fn block_len(&self, index: usize) -> Option<usize> {
        Some(self.rows.get(index)?.lines.as_ref()?.len())
    }
    /// Determine whether visible rows still need original source for this layout.
    pub(super) fn needs_source(&self, index: usize) -> bool {
        self.rows
            .get(index)
            .is_some_and(|row| row.count > 0 && (row.loading || row.lines.is_none()))
    }
    /// Report a visible loading state while keeping the entry's estimated or last-known height.
    pub(super) fn has_loading_rows(&self, start: usize, end: usize) -> bool {
        if start > end || self.rendered_total_lines == 0 {
            return false;
        }
        let first = self.message_anchor(start).map_or(0, |(index, _)| index);
        let last = self
            .message_anchor(end.min(self.rendered_total_lines - 1))
            .map_or(first, |(index, _)| index);
        self.rows
            .get(first..=last)
            .is_some_and(|rows| rows.iter().any(|row| row.loading && row.count > 0))
    }

    pub(super) fn rendered_line_count(&self) -> usize {
        self.rendered_total_lines
    }
    pub(super) fn rendered_line_at(&self, index: usize) -> Option<&RenderedLine> {
        if index >= self.rendered_total_lines {
            return None;
        }
        let (entry, row) = self.message_anchor(index)?;
        self.rows.get(entry)?.lines.as_ref()?.get(row)
    }
    /// Identify the first body row after blank lines and section headings.
    pub(super) fn first_content_row(&self, index: usize) -> usize {
        self.rows
            .get(index)
            .and_then(|row| row.lines.as_ref())
            .map_or(0, |lines| {
                lines.iter().take_while(|line| line.separator).count()
            })
    }
    /// Locate a chronological entry and row against the current prefix sums.
    pub(super) fn message_anchor(&self, line: usize) -> Option<(usize, usize)> {
        let index = self
            .rows
            .partition_point(|row| row.start <= line)
            .checked_sub(1)?;
        Some((index, line.saturating_sub(self.rows[index].start)))
    }
    pub(super) fn line_for_anchor(&self, (index, row): (usize, usize)) -> Option<usize> {
        let entry = self.rows.get(index)?;
        Some(entry.start + row.min(entry.count.saturating_sub(1)))
    }

    pub(super) fn ensure_rendered_range_materialized(
        &mut self,
        messages: &[Message],
        start: usize,
        end: usize,
    ) {
        self.materialize_inputs(messages, start, end);
    }
    pub(super) fn materialize_inputs<S: TranscriptSource + ?Sized>(
        &mut self,
        source: &S,
        start: usize,
        end: usize,
    ) {
        if start > end || self.rendered_total_lines == 0 {
            return;
        }
        let first = self.message_anchor(start).map_or(0, |(index, _)| index);
        let last = self
            .message_anchor(end.min(self.rendered_total_lines - 1))
            .map_or(first, |(index, _)| index);
        for index in first..=last {
            self.materialize(source, index);
        }
    }
    fn materialize<S: TranscriptSource + ?Sized>(&mut self, source: &S, index: usize) {
        let Some(row) = self
            .rows
            .get_mut(index)
            .filter(|row| row.count > 0 && row.lines.is_none())
        else {
            return;
        };
        let Some(input) = source.input(index) else {
            return;
        };
        let Some(plan) = self.entries.plan(index) else {
            return;
        };
        let lines = Arc::new(plan.render_input(input));
        debug_assert_eq!(lines.len(), row.count);
        row.lines = Some(lines.clone());
        self.entries.painted(index, lines);
    }

    #[cfg(test)]
    pub(super) fn snapshot_rendered_lines(&mut self, messages: &[Message]) -> Vec<RenderedLine> {
        if self.rendered_total_lines > 0 {
            self.ensure_rendered_range_materialized(messages, 0, self.rendered_total_lines - 1);
        }
        self.rows
            .iter()
            .filter_map(|row| row.lines.as_ref())
            .flat_map(|lines| lines.iter().cloned())
            .collect()
    }
}

/// Count the same complete plan used by the lazy render cache.
pub(super) fn count_display_block<'a>(
    cache: &mut RenderCountCache<'a>,
    previous: Option<&Message>,
    message: &'a Message,
    width: usize,
    compact: bool,
    theme: TranscriptTheme,
) -> usize {
    plan_display_block(cache, previous, message, width, compact, theme).count(cache, message)
}

impl super::selection::RenderedLineSource for RenderCacheState {
    fn len(&self) -> usize {
        self.rendered_line_count()
    }
    fn get(&self, index: usize) -> Option<&RenderedLine> {
        self.rendered_line_at(index)
    }
}
