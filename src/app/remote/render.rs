//! Draw the existing lazy transcript blocks and composer from client-owned facts.

use carlos_core::session::models::Role;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::Frame;

use super::transcript::Transcript;
use super::Device;
use crate::app::render::{draw_rendered_line, draw_str, fill_rect};
use crate::app::render_cache_state::TranscriptSource;
use crate::app::tool_summary::tool_gutter;
use crate::app::view_models::TerminalSize;
use crate::theme::{role_gutter_fg, role_gutter_symbol, role_row_bg, role_text_style};

impl Device {
    pub(super) fn draw(&mut self, frame: &mut Frame<'_>) {
        if self.metrics_help(frame) {
            return;
        }
        let size = TerminalSize {
            width: frame.area().width as usize,
            height: frame.area().height as usize,
        };
        self.goals.size = size;
        self.queue.size = Some(size);
        self.questions.size = Some(size);
        self.view.layout.update_size(size);
        if self.draw_panel(frame, size) {
            return;
        }
        self.view.tool_output.update_size(size);
        let content_x = if self.view.layout.active.is_flow() {
            1
        } else {
            2
        };
        let width = size.width.saturating_sub(content_x + 1);
        if !self.view.render_cache.matches_view(
            width,
            self.view.tool_output.compact(),
            self.view.layout.active,
        ) {
            self.view.viewport.selection = None;
            self.view.viewport.mouse_drag_mode = crate::app::selection::MouseDragMode::Undecided;
        }
        let layout = self.metrics_layout(size, content_x);
        self.metrics_transcript(frame.buffer_mut(), size, &layout, content_x);
        self.draw_metrics(frame.buffer_mut(), size);
        self.draw_footer(frame.buffer_mut(), size, &layout, content_x);
        self.remember_anchor();
        if self.draw_settings(frame, size) {
            return;
        }
        if self.draw_question_cursor(frame, size) {
            return;
        }
        frame.set_cursor_position((
            layout.cursor_x.min(size.width.saturating_sub(1)) as u16,
            layout.cursor_y.min(size.height.saturating_sub(1)) as u16,
        ));
    }

    fn draw_panel(&mut self, frame: &mut Frame<'_>, size: TerminalSize) -> bool {
        let cursor = if self.tasks.view.show {
            self.draw_tasks(frame.buffer_mut(), size);
            None
        } else if self.view.codex_goals.show {
            Some(self.draw_goal(frame.buffer_mut(), size))
        } else if self.view.codex_queue.show {
            Some(self.draw_queue(frame.buffer_mut(), size))
        } else {
            return false;
        };
        if let Some((x, y)) = cursor {
            frame.set_cursor_position((x as u16, y as u16));
        }
        if !self.tasks.view.show {
            self.draw_metrics(frame.buffer_mut(), size);
        }
        self.draw_settings(frame, size);
        true
    }

    fn draw_question_cursor(&self, frame: &mut Frame<'_>, size: TerminalSize) -> bool {
        if let Some((x, y)) = self.question_cursor(size) {
            frame.set_cursor_position((x as u16, y as u16));
            return true;
        }
        self.selected_approval().is_some()
            && self.task_backend() != Some(carlos_proto::session::Backend::Grok)
    }

    pub(super) fn draw_transcript(
        &mut self,
        buffer: &mut Buffer,
        size: TerminalSize,
        layout: &crate::app::render_input::InputLayout,
        content_x: usize,
    ) {
        let height = layout.msg_bottom;
        let width = size.width.saturating_sub(content_x + 1);
        self.reading.wanted.clear();
        self.reading.visible.clear();
        self.reading.loading = false;
        if let Some(transcript) = Transcript::new(&self.client, &self.session, &self.documents) {
            let cache = &mut self.view.render_cache;
            cache.ensure_inputs(
                &transcript,
                width,
                None,
                self.view.tool_output.compact(),
                self.view.layout.active,
            );
            self.history.align_view(
                self.client.session(&self.session),
                cache,
                &mut self.view.viewport,
                &mut self.reading,
                height,
            );
            let anchor = self.reading.anchor.as_ref();
            if let Some(line) = anchor.and_then(|(id, row)| {
                let position = self.client.session(&self.session)?.entry_position(id)?;
                cache.line_for_anchor((position, *row))
            }) {
                self.view.viewport.scroll_top = line;
            }
            let max_scroll = cache.rendered_line_count().saturating_sub(height);
            if self.view.viewport.auto_follow_bottom {
                self.view.viewport.scroll_top = max_scroll;
            }
            self.view.viewport.scroll_top = self.view.viewport.scroll_top.min(max_scroll);
            let first = self.view.viewport.scroll_top;
            let last = first + height.saturating_sub(1);
            cache.materialize_inputs(&transcript, first, last);
            draw_rows(
                buffer,
                &transcript,
                cache,
                &self.view.viewport,
                size,
                content_x,
                height,
            );
        }
        self.collect_visible(height);
    }
}

fn draw_gutter(
    buffer: &mut Buffer,
    row: usize,
    input: crate::app::transcript_input::RenderInput<'_>,
    first: bool,
    separator: bool,
) {
    if separator {
        return;
    }
    let (glyph, color) = if matches!(input.role, Role::ToolCall | Role::ToolOutput) {
        tool_gutter(input)
    } else {
        (role_gutter_symbol(input.role), role_gutter_fg(input.role))
    };
    let glyph = if first {
        glyph
    } else if input.role == Role::Assistant {
        " "
    } else {
        "┃"
    };
    draw_str(buffer, 0, row, glyph, Style::default().fg(color), 2);
}

fn draw_body_row(
    buffer: &mut Buffer,
    row: usize,
    content_x: usize,
    width: usize,
    line: &crate::app::view_models::RenderedLine,
    selected: Option<(usize, usize)>,
) {
    fill_rect(
        buffer,
        0,
        row,
        buffer.area.width as usize,
        1,
        Style::default().bg(role_row_bg(line.role)),
    );
    draw_rendered_line(
        buffer,
        content_x,
        row,
        width,
        line,
        role_text_style(line.role),
        selected,
    );
}

fn draw_rows(
    buffer: &mut Buffer,
    transcript: &Transcript<'_>,
    cache: &crate::app::render_cache_state::RenderCacheState,
    viewport: &crate::app::viewport_state::ViewportState,
    size: TerminalSize,
    content_x: usize,
    height: usize,
) {
    let first = viewport.scroll_top;
    let width = size.width.saturating_sub(content_x + 1);
    for row in 0..height {
        if let Some(line) = cache.rendered_line_at(first + row) {
            let selected = viewport.selected_cells(first + row, line.cells);
            draw_body_row(buffer, row, content_x, width, line, selected);
            if content_x != 1 {
                if let Some((index, body_row)) = cache.message_anchor(first + row) {
                    if let Some(input) = transcript.row(index).map(|row| row.facts) {
                        draw_gutter(
                            buffer,
                            row,
                            input,
                            body_row == cache.first_content_row(index),
                            line.separator,
                        );
                    }
                }
            }
        }
    }
}
