//! Draw the existing lazy transcript blocks and composer from client-owned facts.

use carlos_core::session::models::Role;
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
        self.view.layout.update_size(size);
        self.view.tool_output.update_size(size);
        let content_x = if self.view.layout.active.is_flow() {
            1
        } else {
            2
        };
        let width = size.width.saturating_sub(content_x + 1);
        let layout = compute_composer_layout(&self.view.input, size, content_x);
        self.draw_transcript(frame.buffer_mut(), size, &layout, content_x);
        let buffer = frame.buffer_mut();
        let info = self
            .client
            .session(&self.session)
            .map(|replica| replica.info());
        let status = self.view.status_hint.clone().unwrap_or_else(|| {
            info.map_or_else(|| "Connecting…".into(), |info| format!("{:?}", info.phase))
        });
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
        draw_composer(buffer, size, &layout, content_x, false, name);
        self.draw_approval(buffer, size);
        frame.set_cursor_position((
            layout.cursor_x.min(size.width.saturating_sub(1)) as u16,
            layout.cursor_y.min(size.height.saturating_sub(1)) as u16,
        ));
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
        if let Some(transcript) = Transcript::new(&self.client, &self.session, &self.documents) {
            let cache = &mut self.view.render_cache;
            cache.ensure_inputs(
                &transcript,
                width,
                None,
                self.view.tool_output.compact(),
                self.view.layout.active,
            );
            let max_scroll = cache.rendered_line_count().saturating_sub(height);
            if self.view.viewport.auto_follow_bottom {
                self.view.viewport.scroll_top = max_scroll;
            }
            self.view.viewport.scroll_top = self.view.viewport.scroll_top.min(max_scroll);
            let first = self.view.viewport.scroll_top;
            cache.materialize_inputs(&transcript, first, first + height.saturating_sub(1));
            for row in 0..height {
                if let Some(line) = cache.rendered_line_at(first + row) {
                    draw_body_row(buffer, row, content_x, width, line);
                    if !self.view.layout.active.is_flow() {
                        if let Some((index, body_row)) = cache.message_anchor(first + row) {
                            if let Some(input) = transcript.input(index) {
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
        None,
    );
}
