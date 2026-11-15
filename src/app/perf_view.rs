//! Paint offline exported rows with the shared transcript cache, composer and borrowed body widget.

#[path = "perf_view_source.rs"]
mod source;

#[cfg(test)]
#[path = "perf_view_tests.rs"]
mod tests;

use carlos_core::perf_fixture::{PerfData, PerfRow};
use carlos_core::session::entry_id::EntryId;
use carlos_proto::session::{Entry, EntryFormat, EntryKind};
use ratatui::style::Style;
use ratatui::Frame;
use ratatui_textarea::TextArea;
use uuid::Uuid;

use super::input::make_input_area;
use super::layout_state::LayoutState;
use super::notifications::animation_tick;
use super::render::{draw_composer, draw_status_separator, fill_rect};
use super::render_cache_state::RenderCacheState;
use super::render_input::compute_composer_layout;
use super::render_transcript::{draw_rows, TranscriptFrame};
use super::status_line::{draw_labels, fit_model, place, place_context, sweep_status_line, Label};
use super::tool_output_state::ToolOutputState;
use super::view_models::TerminalSize;
use super::viewport_state::ViewportState;
use crate::theme::{COLOR_STEP2, COLOR_STEP8};

/// Own exported offline rows and the local fields used by the existing draw benchmarks.
pub(super) struct PerfView {
    pub(super) rows: Vec<PerfRow>,
    pub(super) view: PerfLocal,
    pub(super) working: bool,
}

/// Keep only offline terminal fields, without retaining the embedded panel/runtime aggregate.
pub(super) struct PerfLocal {
    pub(super) layout: LayoutState,
    pub(super) tool_output: ToolOutputState,
    pub(super) input: TextArea<'static>,
    pub(super) render_cache: RenderCacheState,
    pub(super) viewport: ViewportState,
}

impl PerfView {
    /// Adopt complete originals after core has dropped its private fixture owner.
    pub(super) fn new(data: PerfData) -> Self {
        Self {
            rows: data.rows,
            view: PerfLocal {
                layout: LayoutState::default(),
                tool_output: ToolOutputState::default(),
                input: make_input_area(),
                render_cache: RenderCacheState::new(),
                viewport: ViewportState::new(),
            },
            working: false,
        }
    }

    /// Keep the same theme-specific left padding as the interactive transcript.
    pub(super) fn content_x(&self, size: TerminalSize) -> usize {
        if self.view.layout.uses_flow(size) {
            1
        } else {
            2
        }
    }

    /// Reserve the existing blank column at the right edge of each body row.
    pub(super) fn transcript_width(&self, size: TerminalSize) -> usize {
        size.width.saturating_sub(self.content_x(size) + 1)
    }

    /// Count changed exported entries through the shared lazy render cache.
    pub(super) fn ensure_rendered_lines(&mut self, width: usize) {
        self.view.render_cache.ensure_inputs(
            self.rows.as_slice(),
            width,
            None,
            self.view.tool_output.compact(),
            self.view.layout.active,
        );
    }

    /// Return the complete wrapped-row count, including headings and separators.
    pub(super) fn rendered_line_count(&self) -> usize {
        self.view.render_cache.rendered_line_count()
    }

    /// Add the same empty assistant row used by the live-append benchmark.
    pub(super) fn append_answer(&mut self) -> usize {
        let index = self.rows.len();
        let id = format!("{}-0000000000000000", Uuid::new_v4().simple())
            .parse()
            .expect("offline fixture id");
        self.rows.push(PerfRow {
            entry: Entry {
                id,
                kind: EntryKind::Answer,
                format: EntryFormat::Plain,
                text: String::new(),
                file_path: None,
                source: None,
                done: true,
                tool: None,
                operation: None,
            },
            source: None,
        });
        self.dirty(index);
        index
    }

    /// Grow only the benchmark answer and invalidate its saved rows.
    pub(super) fn append_text(&mut self, index: usize, text: &str) {
        self.rows[index].entry.text.push_str(text);
        self.dirty(index);
    }

    fn dirty(&mut self, index: usize) {
        let id = EntryId::from_wire(&self.rows[index].entry.id);
        self.view
            .render_cache
            .invalidate_entry(index, &super::render_entry_cache::RenderKey::Entry(id));
        self.view
            .render_cache
            .mark_transcript_dirty_from(self.rows.len(), index);
    }

    /// Paint the shared transcript body and composer with offline status facts.
    pub(super) fn draw(&mut self, frame: &mut Frame<'_>) {
        let size = TerminalSize {
            width: frame.area().width as usize,
            height: frame.area().height as usize,
        };
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.view.layout.update_size(size);
        self.view.tool_output.update_size(size);
        let x = self.content_x(size);
        let width = self.transcript_width(size);
        self.ensure_rendered_lines(width);
        let layout = compute_composer_layout(&self.view.input, size, x);
        let height = layout.msg_bottom;
        self.sync_viewport(height);
        let buffer = frame.buffer_mut();
        self.draw_body(buffer, size, height, width, x);
        draw_composer(buffer, size, &layout, x, false, None);
        self.draw_status(buffer, size, layout.input_top.saturating_sub(2));
        frame.set_cursor_position((layout.cursor_x as u16, layout.cursor_y as u16));
    }

    fn sync_viewport(&mut self, height: usize) {
        let count = self.rendered_line_count();
        let max_scroll = count.saturating_sub(height);
        self.view.viewport.scroll_top = self.view.viewport.scroll_top.min(max_scroll);
        if self.view.viewport.auto_follow_bottom {
            self.view.viewport.scroll_top = max_scroll;
        }
        if height > 0 && count > 0 {
            let first = self.view.viewport.scroll_top;
            let last = (first + height - 1).min(count - 1);
            self.view
                .render_cache
                .materialize_inputs(self.rows.as_slice(), first, last);
        }
    }

    fn draw_body(
        &self,
        buffer: &mut ratatui::buffer::Buffer,
        size: TerminalSize,
        height: usize,
        width: usize,
        x: usize,
    ) {
        fill_rect(
            buffer,
            0,
            0,
            size.width,
            size.height,
            Style::default().bg(COLOR_STEP2),
        );
        draw_rows(
            buffer,
            &TranscriptFrame {
                source: self.rows.as_slice(),
                cache: &self.view.render_cache,
                viewport: &self.view.viewport,
                flow: self.view.layout.active.is_flow(),
                content_x: x,
                active: None,
            },
            height,
            width,
        );
    }

    fn draw_status(&self, buffer: &mut ratatui::buffer::Buffer, size: TerminalSize, y: usize) {
        let mut edge = size.width.saturating_sub(1);
        let mut labels = Vec::new();
        place_context(&mut labels, &mut edge, None, false, 0);
        let model = fit_model("model?", edge.saturating_sub(2));
        place(&mut labels, &mut edge, Label::plain(model, COLOR_STEP8));
        draw_status_separator(buffer, y, edge, false);
        draw_labels(buffer, y, &labels);
        if self.working {
            let width = if self.view.layout.uses_flow(size) || size.is_small() || edge < 8 {
                size.width.saturating_sub(1)
            } else {
                edge
            };
            sweep_status_line(buffer, y, width, false, animation_tick());
        }
    }
}
