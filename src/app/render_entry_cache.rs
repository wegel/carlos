//! Retain entry layouts independently of source-window residency and chronological position.

#[path = "render_row_window.rs"]
mod window;

pub(super) use self::window::PaintedRows;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use carlos_core::session::entry_id::EntryId;
use carlos_proto::output::OutputRevision;
use carlos_proto::submissions::ClientOperation;
use ratatui::style::Style;

use super::flow_transcript::EntryContext;
use super::layout_state::TranscriptTheme;
use super::text::{slice_by_cells, visual_width};
use super::transcript_block::{plan_context_block, visible_input, BlockPlan};
use super::transcript_input::RenderInput;
use super::transcript_styles::RenderCountCache;
use super::view_models::{RenderedLine, StyledSegment};
use crate::theme::COLOR_STEP8;

/// Keep an optimistic prompt distinct from every authoritative shared row.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum RenderKey {
    Entry(EntryId),
    Echo(ClientOperation),
}

/// Borrow current facts separately from an optional complete original source.
pub(super) struct RowSource<'a> {
    pub key: RenderKey,
    pub generation: Option<OutputRevision>,
    pub input: Option<RenderInput<'a>>,
    pub facts: RenderInput<'a>,
}

/// Each entry remembers which display choices produced its saved count.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct RowFormat {
    pub width: usize,
    pub compact: bool,
    pub theme: TranscriptTheme,
    pub hidden: bool,
}

/// The chronological index shares plans and painted rows without copying their strings.
#[derive(Clone)]
pub(super) struct PreparedRow {
    pub count: usize,
    pub lines: Option<PaintedRows>,
    pub context: EntryContext,
    pub loading: bool,
}

struct EntryMemo {
    plan: Option<BlockPlan>,
    generation: Option<OutputRevision>,
    format: RowFormat,
    previous: Option<EntryContext>,
    row: PreparedRow,
    stale: bool,
}

struct CachedEntry {
    key: RenderKey,
    memo: EntryMemo,
}

/// Reuse chronological entries directly; build an id lookup only when a prepend/reorder needs it.
pub(super) struct ReusedEntries {
    ordered: VecDeque<CachedEntry>,
    by_id: Option<HashMap<RenderKey, EntryMemo>>,
}

impl ReusedEntries {
    fn take(&mut self, key: &RenderKey) -> Option<EntryMemo> {
        if self.ordered.front().is_some_and(|entry| entry.key == *key) {
            return self.ordered.pop_front().map(|entry| entry.memo);
        }
        if self.ordered.is_empty() && self.by_id.is_none() {
            return None;
        }
        self.by_id
            .get_or_insert_with(|| {
                self.ordered
                    .drain(..)
                    .map(|entry| (entry.key, entry.memo))
                    .collect()
            })
            .remove(key)
    }
}

#[derive(Default)]
pub(super) struct EntryMemos {
    entries: Vec<CachedEntry>,
}

impl EntryMemos {
    pub fn reserve(&mut self, length: usize) {
        self.entries
            .reserve(length.saturating_sub(self.entries.len()));
    }

    /// Refresh every saved row after an authoritative snapshot replaces its facts.
    pub fn invalidate_all(&mut self) {
        self.invalidate_suffix(0);
    }

    /// Refresh an old positional suffix even if replacement facts have moved those ids.
    pub fn invalidate_suffix(&mut self, index: usize) {
        for entry in self.entries.iter_mut().skip(index) {
            entry.memo.stale = true;
        }
    }

    pub fn invalidate(&mut self, index: usize, key: &RenderKey) {
        if self
            .entries
            .get(index)
            .is_some_and(|entry| entry.key == *key)
        {
            self.entries[index].memo.stale = true;
        } else if let Some(entry) = self.entries.iter_mut().find(|entry| entry.key == *key) {
            // Facts may already contain prepended history while the last drawn index is older.
            entry.memo.stale = true;
        }
    }

    pub fn begin_suffix(&mut self, index: usize) -> ReusedEntries {
        ReusedEntries {
            ordered: self.entries.split_off(index.min(self.entries.len())).into(),
            by_id: None,
        }
    }

    pub fn prepare<'a>(
        &mut self,
        source: RowSource<'a>,
        format: RowFormat,
        previous: Option<EntryContext>,
        prepared: Option<PreparedRow>,
        cache: &mut RenderCountCache<'a>,
        reused: &mut ReusedEntries,
    ) -> PreparedRow {
        let mut memo = reused.take(&source.key);
        let row = if let Some(row) = prepared {
            memo = Some(EntryMemo {
                plan: None,
                generation: source.generation,
                format,
                previous,
                row: row.clone(),
                stale: false,
            });
            row
        } else if let Some(input) = source.input {
            if memo.as_ref().is_none_or(|memo| {
                memo.stale
                    || memo.format != format
                    || memo.generation != source.generation
                    || memo.previous != previous
            }) {
                memo = Some(EntryMemo::ready(
                    input,
                    source.generation,
                    format,
                    previous,
                    cache,
                ));
            }
            memo.as_ref().expect("prepared source").row.clone()
        } else {
            missing_row(
                memo.as_ref(),
                source.generation,
                source.facts,
                format,
                previous,
            )
        };
        let memo = memo.unwrap_or_else(|| EntryMemo {
            plan: None,
            generation: source.generation,
            format,
            previous,
            row: row.clone(),
            stale: true,
        });
        self.entries.push(CachedEntry {
            key: source.key,
            memo,
        });
        row
    }

    /// Borrow a saved plan only while painting its chronological entry.
    pub fn plan(&self, index: usize) -> Option<&BlockPlan> {
        self.entries.get(index)?.memo.plan.as_ref()
    }

    pub fn painted(&mut self, index: usize, lines: Arc<Vec<RenderedLine>>) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.memo.row.lines = Some(PaintedRows::whole(lines));
        }
    }
}

impl EntryMemo {
    fn ready<'a>(
        input: RenderInput<'a>,
        generation: Option<OutputRevision>,
        format: RowFormat,
        previous: Option<EntryContext>,
        cache: &mut RenderCountCache<'a>,
    ) -> Self {
        let (row, plan) = ready_row(input, format, previous, cache);
        Self {
            generation,
            format,
            previous,
            row,
            plan,
            stale: false,
        }
    }
}

fn ready_row<'a>(
    input: RenderInput<'a>,
    format: RowFormat,
    previous: Option<EntryContext>,
    cache: &mut RenderCountCache<'a>,
) -> (PreparedRow, Option<BlockPlan>) {
    let context = EntryContext::new(input, format.width, format.theme);
    if format.hidden || !visible_input(input) || format.width < 8 {
        return (
            PreparedRow {
                count: 0,
                lines: None,
                context,
                loading: false,
            },
            None,
        );
    }
    let plan = plan_context_block(
        cache,
        previous,
        context,
        input,
        format.width,
        format.compact,
        format.theme,
    );
    let count = plan.count_input(cache, input);
    (
        PreparedRow {
            count,
            lines: None,
            context,
            loading: false,
        },
        Some(plan),
    )
}

fn missing_row(
    memo: Option<&EntryMemo>,
    generation: Option<OutputRevision>,
    facts: RenderInput<'_>,
    format: RowFormat,
    previous: Option<EntryContext>,
) -> PreparedRow {
    if let Some(memo) = memo.filter(|memo| memo.format == format && memo.generation == generation) {
        let mut row = memo.row.clone();
        row.loading = memo.stale || memo.previous != previous || row.lines.is_none();
        if row.loading && !format.hidden && format.width >= 8 {
            row.count = row.count.max(1);
        }
        if row.lines.is_none() {
            row.lines = Some(loading_lines(facts, format.width));
        }
        return row;
    }
    let count = if format.hidden || format.width < 8 {
        0
    } else {
        memo.map_or(1, |memo| memo.row.count.max(1))
    };
    PreparedRow {
        count,
        lines: Some(loading_lines(facts, format.width)),
        context: EntryContext::new(facts, format.width, format.theme),
        loading: true,
    }
}

fn loading_lines(facts: RenderInput<'_>, width: usize) -> PaintedRows {
    let text = match facts.label {
        Some(label) => format!("{label} · loading source…"),
        None => "Loading original text…".into(),
    };
    let text = slice_by_cells(&text, 0, width);
    PaintedRows::whole(Arc::new(vec![RenderedLine {
        cells: visual_width(&text),
        text: text.clone(),
        role: facts.role,
        styled_segments: vec![StyledSegment {
            text,
            style: Style::default().fg(COLOR_STEP8),
        }],
        separator: false,
        soft_wrap_to_next: false,
    }]))
}
