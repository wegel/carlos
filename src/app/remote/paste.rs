//! Route pasted text to the active local editor before considering mobile mouse fallbacks.

use super::Device;
use crate::app::mobile_mouse;
use crate::app::render_input::normalize_pasted_text;

impl Device {
    pub(super) fn paste_text(&mut self, text: String) {
        if self.view.runtime_settings.show_model_settings {
            self.settings_paste(&normalize_pasted_text(&text));
        } else if self.tasks.view.show {
            // Task panels consume paste without changing a hidden composer.
        } else if self.view.codex_goals.show {
            self.goal_paste(&normalize_pasted_text(&text));
        } else if self.view.codex_queue.show {
            self.queue_paste(&normalize_pasted_text(&text));
        } else if self.question_paste(&normalize_pasted_text(&text)) {
        } else if mobile_mouse::consume_paste(
            &mut self.view.viewport,
            self.view.input.is_empty(),
            &text,
        ) {
            self.note_mobile_scroll();
        } else {
            self.view.reset_esc_chord();
            self.cancel_prompt_read_for_edit();
            self.view.input.insert_str(normalize_pasted_text(&text));
        }
    }
}
