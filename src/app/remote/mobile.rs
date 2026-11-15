//! Decode mobile terminal mouse fallbacks using only local viewport and composer state.

use crossterm::event::{KeyCode, KeyEvent};

use super::Device;
use crate::app::mobile_mouse::{
    consume_mobile_mouse_char, is_mobile_mouse_key_candidate, take_mobile_mouse_buffer,
    MobileMouseConsume,
};

impl Device {
    pub(super) fn mobile_key(&mut self, key: KeyEvent) -> bool {
        if is_mobile_mouse_key_candidate(&self.view.viewport, key.code, key.modifiers) {
            if let KeyCode::Char(ch) = key.code {
                let was_plain = self.view.viewport.mobile_plain_pending_coords;
                match consume_mobile_mouse_char(&mut self.view.viewport, ch) {
                    MobileMouseConsume::PassThrough => {}
                    MobileMouseConsume::Consumed => {
                        self.note_mobile_scroll();
                        return true;
                    }
                    MobileMouseConsume::Emit(text) => {
                        if !was_plain {
                            self.insert_mobile_text(text);
                        }
                        return true;
                    }
                }
            }
        } else if let Some(text) = take_mobile_mouse_buffer(&mut self.view.viewport) {
            if !self.view.viewport.mobile_plain_pending_coords
                && !self.view.viewport.mobile_plain_suppress_coords
            {
                self.insert_mobile_text(text);
            }
            self.view.viewport.mobile_plain_pending_coords = false;
            self.view.viewport.mobile_plain_suppress_coords = false;
        }
        false
    }

    fn insert_mobile_text(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        self.view.reset_esc_chord();
        self.cancel_prompt_read_for_edit();
        self.view.input.insert_str(text);
    }

    pub(super) fn note_mobile_scroll(&mut self) {
        self.reading.older_requested |=
            self.view.viewport.scroll_top == 0 && !self.view.viewport.auto_follow_bottom;
    }
}

#[cfg(test)]
#[path = "mobile_mouse_tests.rs"]
mod tests;
