//! Scroll the existing help painter without sending keys or paste to a hidden editor.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;

use super::Device;
use crate::app::overlay_render::{draw_help_frame, help_scroll_limit};
use crate::app::view_models::TerminalSize;

impl Device {
    pub(super) fn help_key(&mut self, key: KeyEvent) -> bool {
        if !self.view.viewport.show_help {
            if key.code != KeyCode::Char('?')
                || !self.view.input.is_empty()
                || self.view.runtime_settings.show_model_settings
                || self.view.codex_goals.show
                || self.view.codex_queue.show
                || self.tasks.view.show
                || self.selected_approval().is_some()
            {
                return false;
            }
            self.view.viewport.show_help = true;
            self.view.viewport.help_scroll = 0;
            return true;
        }
        let maximum = crossterm::terminal::size()
            .map(|(width, height)| {
                help_scroll_limit(TerminalSize {
                    width: width as usize,
                    height: height as usize,
                })
            })
            .unwrap_or(0);
        let scroll = &mut self.view.viewport.help_scroll;
        *scroll = (*scroll).min(maximum);
        match key.code {
            KeyCode::Esc | KeyCode::Char('?') => self.view.viewport.show_help = false,
            KeyCode::Down => *scroll = scroll.saturating_add(1).min(maximum),
            KeyCode::Up => *scroll = scroll.saturating_sub(1),
            KeyCode::PageDown => *scroll = scroll.saturating_add(5).min(maximum),
            KeyCode::PageUp => *scroll = scroll.saturating_sub(5),
            KeyCode::Home => *scroll = 0,
            KeyCode::End => *scroll = maximum,
            _ => {}
        }
        true
    }

    pub(super) fn draw_help(&self, frame: &mut Frame<'_>) -> bool {
        if !self.view.viewport.show_help {
            return false;
        }
        let size = TerminalSize {
            width: frame.area().width as usize,
            height: frame.area().height as usize,
        };
        draw_help_frame(frame.buffer_mut(), size, self.view.viewport.help_scroll);
        true
    }
}
