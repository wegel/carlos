//! Draw the existing lazy transcript blocks and composer from client-owned facts.

use ratatui::buffer::Buffer;
use ratatui::Frame;

use super::transcript::Transcript;
use super::Device;
use crate::app::render_transcript::{draw_rows, TranscriptFrame};
use crate::app::view_models::TerminalSize;

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
            let active = self.client.session(&self.session).and_then(|replica| {
                let id = replica.info().status.as_ref()?.active_reasoning.as_ref()?;
                replica.entry_position(id)
            });
            let frame = TranscriptFrame {
                source: &transcript,
                cache,
                viewport: &self.view.viewport,
                flow: self.view.layout.active.is_flow(),
                content_x,
                active,
            };
            draw_rows(buffer, &frame, height, width);
        }
        self.collect_visible(height);
    }
}
