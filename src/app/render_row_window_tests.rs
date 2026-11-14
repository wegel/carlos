//! Keep viewport row addresses and saved counts exact when only part of a large block is painted.

use std::sync::Arc;

use carlos_core::session::models::{Message, Role};
use carlos_core::session::state::SessionState;
use carlos_proto::output::OutputRevision;
use ratatui::style::Style;

use super::{RenderCacheState, TranscriptSource};
use crate::app::flow_transcript::EntryContext;
use crate::app::layout_state::TranscriptTheme;
use crate::app::render_entry_cache::{PaintedRows, PreparedRow, RenderKey, RowFormat, RowSource};
use crate::app::view_models::RenderedLine;

struct PagedSource {
    entries: Vec<Message>,
    frame: Option<PreparedRow>,
    generation: OutputRevision,
}

impl PagedSource {
    fn new(first: usize) -> Self {
        let mut core = SessionState::new("render-window".into(), Default::default(), false);
        core.append_message(Role::User, "prefix 🌍");
        core.append_message(Role::ToolOutput, "");
        let mut source = Self {
            entries: core.messages().to_vec(),
            frame: None,
            generation: OutputRevision::new(1),
        };
        source.frame = Some(source.frame(first));
        source
    }

    fn frame(&self, first: usize) -> PreparedRow {
        let prefix = Arc::new(vec![line("heading", true)]);
        let rows = Arc::new(
            (first..first + 50)
                .map(|row| line(&format!("row {row}"), false))
                .collect(),
        );
        PreparedRow {
            count: 2_000_001,
            lines: Some(PaintedRows::window(Some(prefix), first, rows)),
            context: EntryContext::new((&self.entries[1]).into(), 28, TranscriptTheme::Gutter),
            loading: false,
        }
    }
}

impl TranscriptSource for PagedSource {
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn row(&self, index: usize) -> Option<RowSource<'_>> {
        let entry = self.entries.get(index)?;
        Some(RowSource {
            key: RenderKey::Entry(entry.id),
            generation: Some(self.generation),
            input: (index == 0).then(|| entry.into()),
            facts: entry.into(),
        })
    }
    fn prepared(
        &self,
        index: usize,
        format: RowFormat,
        _previous: Option<EntryContext>,
    ) -> Option<PreparedRow> {
        (index == 1
            && format.width == 28
            && !format.compact
            && format.theme == TranscriptTheme::Gutter)
            .then(|| self.frame.clone())
            .flatten()
    }
}

fn line(text: &str, separator: bool) -> RenderedLine {
    RenderedLine {
        text: text.into(),
        role: Role::ToolOutput,
        styled_segments: vec![crate::app::view_models::StyledSegment {
            text: text.into(),
            style: Style::default(),
        }],
        separator,
        cells: text.len(),
        soft_wrap_to_next: false,
    }
}

fn draw(cache: &mut RenderCacheState, source: &PagedSource, width: usize) {
    cache.ensure_inputs(source, width, None, false, TranscriptTheme::Gutter);
}

#[test]
fn a_large_block_keeps_absolute_rows_and_requests_only_missing_visible_intervals() {
    let source = PagedSource::new(1_000_000);
    let mut cache = RenderCacheState::new();
    draw(&mut cache, &source, 28);
    let start = cache.block_start(1).expect("large block");
    let first = start + 1_000_000;
    assert_eq!(cache.block_len(1), Some(2_000_001));
    assert_eq!(cache.first_content_row(1), 1);
    assert_eq!(
        cache.rendered_line_at(start).expect("heading").text,
        "heading"
    );
    assert!(cache.rendered_line_at(start + 999_999).is_none());
    assert_eq!(
        cache
            .rendered_line_at(first + 49)
            .expect("last visible")
            .text,
        "row 1000049"
    );
    assert!(cache.rendered_line_at(first + 50).is_none());
    assert!(!cache.needs_source(1, first, first + 49));
    assert!(!cache.has_loading_rows(first, first + 49));
    assert!(cache.needs_source(1, first + 50, first + 99));
    assert!(cache.has_loading_rows(first + 50, first + 99));
    cache.materialize_inputs(&source, first, first + 49);
    let rows = cache.rows[1].lines.as_ref().expect("bounded rows");
    assert_eq!(rows.retained().count(), 51);
}

#[test]
fn a_new_window_replaces_old_painted_rows_without_changing_the_saved_scroll_address() {
    let mut source = PagedSource::new(1_000_000);
    let mut cache = RenderCacheState::new();
    draw(&mut cache, &source, 28);
    let anchor = (1, 1_000_020);
    let before = cache.line_for_anchor(anchor).expect("saved row");
    source.frame = Some(source.frame(1_000_020));
    cache.invalidate_entry(1, &RenderKey::Entry(source.entries[1].id));
    cache.mark_transcript_dirty_from(2, 1);
    draw(&mut cache, &source, 28);
    assert_eq!(cache.line_for_anchor(anchor), Some(before));
    assert_eq!(
        cache.rendered_line_at(before).expect("new window").text,
        "row 1000020"
    );
    assert!(cache.rendered_line_at(before - 1).is_none());
    source.frame = None;
    cache.mark_transcript_dirty_from(2, 1);
    draw(&mut cache, &source, 28);
    assert_eq!(cache.line_for_anchor(anchor), Some(before));
    assert_eq!(
        cache
            .rendered_line_at(before)
            .expect("accepted rows stay visible")
            .text,
        "row 1000020"
    );
}

#[test]
fn a_replacement_or_resize_cannot_reuse_rows_from_a_different_source_or_width() {
    let mut source = PagedSource::new(1_000_000);
    let mut cache = RenderCacheState::new();
    draw(&mut cache, &source, 28);
    source.frame = None;
    source.generation = OutputRevision::new(2);
    cache.mark_transcript_dirty_from(2, 1);
    draw(&mut cache, &source, 28);
    let start = cache.block_start(1).expect("block");
    assert!(cache.rendered_line_at(start + 1_000_000).is_none());
    assert!(cache
        .rendered_line_at(start)
        .expect("loading row")
        .text
        .contains("Loading"));
    assert!(cache.needs_source(1, start, start + 49));
    source.frame = Some(source.frame(1_000_000));
    cache.mark_transcript_dirty_from(2, 1);
    draw(&mut cache, &source, 28);
    draw(&mut cache, &source, 80);
    assert!(cache.rendered_line_at(start + 1_000_000).is_none());
    assert!(cache.has_loading_rows(start, start + 49));
}
