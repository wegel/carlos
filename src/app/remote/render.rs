//! Draw the existing lazy transcript blocks and composer from client-owned facts.

use carlos_core::session::models::Role;
use carlos_proto::session::Availability;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::Frame;

use super::transcript::Transcript;
use super::Device;
use crate::app::render::{draw_composer, draw_rendered_line, draw_str, fill_rect};
use crate::app::render_cache_state::TranscriptSource;
use crate::app::render_input::compute_composer_layout;
use crate::app::tool_summary::tool_gutter;
use crate::app::view_models::TerminalSize;
use crate::theme::{role_gutter_fg, role_gutter_symbol, role_row_bg, role_text_style, COLOR_TEXT};

impl Device {
    pub(super) fn draw(&mut self, frame: &mut Frame<'_>) {
        let size = TerminalSize {
            width: frame.area().width as usize,
            height: frame.area().height as usize,
        };
        self.goals.size = size;
        self.queue.size = Some(size);
        self.view.layout.update_size(size);
        if self.tasks.view.show {
            self.draw_tasks(frame.buffer_mut(), size);
            return;
        }
        if self.view.codex_goals.show {
            let (x, y) = self.draw_goal(frame.buffer_mut(), size);
            frame.set_cursor_position((x as u16, y as u16));
            return;
        }
        if self.view.codex_queue.show {
            let (x, y) = self.draw_queue(frame.buffer_mut(), size);
            frame.set_cursor_position((x as u16, y as u16));
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
        let layout = compute_composer_layout(&self.view.input, size, content_x);
        self.draw_transcript(frame.buffer_mut(), size, &layout, content_x);
        self.draw_footer(frame.buffer_mut(), size, &layout, content_x);
        self.remember_anchor();
        frame.set_cursor_position((
            layout.cursor_x.min(size.width.saturating_sub(1)) as u16,
            layout.cursor_y.min(size.height.saturating_sub(1)) as u16,
        ));
    }

    fn draw_footer(
        &self,
        buffer: &mut Buffer,
        size: TerminalSize,
        layout: &crate::app::render_input::InputLayout,
        content_x: usize,
    ) {
        let width = size.width.saturating_sub(content_x + 1);
        let info = self
            .client
            .session(&self.session)
            .map(|replica| replica.info());
        let status = self.view.status_hint.clone().unwrap_or_else(|| {
            if self.waiting_ready {
                return "Waiting for the session to become ready; your draft is kept. Press Enter again when ready".into();
            }
            if self.reading.loading {
                return "Loading visible source… · F6 retries failed reads".into();
            }
            info.map_or_else(
                || "Connecting…".into(),
                |info| {
                    if info.availability == Availability::Ready
                        && !self.client.current_session_is_ready(&self.session)
                    {
                        "Restoring session…".into()
                    } else if info.availability == Availability::Ready {
                        format!("{:?}", info.phase)
                    } else {
                        format!("{:?}", info.availability)
                    }
                },
            )
        });
        let status = self.dictation_status(self.task_status(self.queue_status(status)));
        crate::app::render::draw_status_separator(
            buffer,
            layout.input_top.saturating_sub(2),
            size.width.saturating_sub(1),
            false,
        );
        draw_str(
            buffer,
            content_x,
            layout.input_top.saturating_sub(2),
            &status,
            Style::default().fg(COLOR_TEXT),
            width,
        );
        let name = self
            .view
            .input
            .is_empty()
            .then(|| info.and_then(|info| info.name.as_deref()))
            .flatten();
        draw_composer(buffer, size, layout, content_x, false, name);
        self.draw_approval(buffer, size);
    }

    fn draw_transcript(
        &mut self,
        buffer: &mut Buffer,
        size: TerminalSize,
        layout: &crate::app::render_input::InputLayout,
        content_x: usize,
    ) {
        let height = layout.msg_bottom;
        let width = size.width.saturating_sub(content_x + 1);
        let anchor = self.reading.anchor.as_ref();
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
            for row in 0..height {
                if let Some(line) = cache.rendered_line_at(first + row) {
                    let selected = self.view.viewport.selected_cells(first + row, line.cells);
                    draw_body_row(buffer, row, content_x, width, line, selected);
                    if !self.view.layout.active.is_flow() {
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
