//! AppState impl: text input editing, escape-chord, history navigation, and rewind mode.

use std::time::{Duration, Instant};

use super::models::TerminalSize;
use super::render::{compute_input_layout, textarea_input_from_key};
use super::state::AppState;
use super::transcript_render::{build_rendered_lines_with_hidden, transcript_content_width};
use super::MSG_TOP;

impl AppState {
    // --- Input handling ---

    pub(super) fn input_is_empty(&self) -> bool {
        self.input.is_empty()
    }

    pub(super) fn input_text(&self) -> String {
        self.input.lines().join("\n")
    }

    pub(super) fn clear_input(&mut self) {
        self.input = super::input::make_input_area();
        self.reset_input_history_navigation();
    }

    pub(super) fn set_input_text(&mut self, text: &str) {
        self.input = super::input::make_input_area();
        if !text.is_empty() {
            let _ = self.input.insert_str(text);
        }
    }

    pub(super) fn reset_input_history_navigation(&mut self) {
        self.input_history.reset_navigation();
    }

    pub(super) fn reset_esc_chord(&mut self) {
        self.esc_armed_at = None;
    }

    pub(super) fn expire_esc_chord(&mut self, now: Instant) {
        const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);
        if let Some(armed_at) = self.esc_armed_at {
            if now.duration_since(armed_at) > ESC_CHORD_WINDOW {
                self.esc_armed_at = None;
            }
        }
    }

    pub(super) fn register_escape_press(&mut self, now: Instant) -> bool {
        const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);
        if let Some(armed_at) = self.esc_armed_at {
            if now.duration_since(armed_at) <= ESC_CHORD_WINDOW {
                self.esc_armed_at = None;
                return true;
            }
        }
        self.esc_armed_at = Some(now);
        false
    }

    // --- Rewind ---

    pub(super) fn enter_rewind_mode(&mut self) {
        if !self.input_history.enter_rewind_mode(self.input_text()) {
            return;
        }
        self.viewport.auto_follow_bottom = false;
        let _ = self.navigate_input_history_up();
    }

    pub(super) fn exit_rewind_mode_restore(&mut self) {
        let Some(draft) = self.input_history.exit_rewind_mode_restore() else {
            return;
        };
        self.viewport.auto_follow_bottom = true;
        self.set_input_text(&draft);
    }

    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.input_history.clear_rewind_mode_state();
        self.viewport.auto_follow_bottom = true;
    }

    pub(super) fn rewind_fork_from_message_idx(&mut self, message_idx: Option<usize>) {
        let Some(idx) = message_idx else {
            return;
        };
        if idx > self.messages.len() {
            return;
        }

        self.messages.truncate(idx);
        self.viewport.selection = None;
        self.viewport.mouse_drag_mode = super::selection::MouseDragMode::Undecided;
        self.viewport.auto_follow_bottom = true;
        self.viewport.scroll_top = self.viewport.scroll_top.min(self.messages.len());
        self.agent_item_to_index.retain(|_, msg_idx| *msg_idx < idx);
        self.turn_diff_to_index.retain(|_, msg_idx| *msg_idx < idx);
        self.command_render_overrides.clear();
        self.input_history.clear_message_indices_from(idx);
        self.mark_transcript_dirty_from(idx);
    }

    pub(super) fn mark_user_turn_submitted(&mut self) {
        self.ralph_runtime.mark_user_turn_submitted();
    }

    pub(super) fn push_input_history(&mut self, text: &str) {
        self.input_history.push_history(text);
    }

    pub(super) fn record_input_history(&mut self, text: &str, message_idx: Option<usize>) {
        self.input_history.record_history(text, message_idx);
    }

    pub(super) fn navigate_input_history_up(&mut self) -> bool {
        let Some(text) = self.input_history.navigate_up(self.input_text()) else {
            return false;
        };
        self.set_input_text(&text);
        true
    }

    pub(super) fn navigate_input_history_down(&mut self) -> bool {
        let Some(text) = self.input_history.navigate_down() else {
            return false;
        };
        self.set_input_text(&text);
        true
    }

    pub(super) fn rewind_selected_message_idx(&self) -> Option<usize> {
        self.input_history.rewind_selected_message_idx()
    }

    pub(super) fn rewind_selected_history_index(&self) -> Option<usize> {
        self.input_history.rewind_selected_history_index()
    }

    pub(super) fn submitted_turn_count(&self) -> usize {
        self.input_history.history_len()
    }

    pub(super) fn align_rewind_scroll_to_selected_prompt(&mut self, size: TerminalSize) {
        if !self.rewind_mode() {
            return;
        }
        let Some(msg_idx) = self.rewind_selected_message_idx() else {
            return;
        };
        if self.messages.is_empty() {
            return;
        }
        let width = transcript_content_width(size);
        if width == 0 {
            return;
        }
        let upto = msg_idx.min(self.messages.len().saturating_sub(1));
        let rendered_upto =
            build_rendered_lines_with_hidden(&self.messages[..=upto], width, Some(msg_idx));
        if rendered_upto.is_empty() {
            return;
        }
        let input_layout = compute_input_layout(self, size);
        let msg_height = if input_layout.msg_bottom >= MSG_TOP {
            input_layout.msg_bottom - MSG_TOP + 1
        } else {
            0
        };
        if msg_height == 0 {
            return;
        }
        let target_line = rendered_upto.len().saturating_sub(1);
        self.viewport.scroll_top = target_line.saturating_sub(msg_height.saturating_sub(1));
    }

    pub(super) fn input_apply_key(&mut self, key: crossterm::event::KeyEvent) {
        self.reset_esc_chord();
        if !self.rewind_mode() {
            self.reset_input_history_navigation();
        }
        let _ = self.input.input(textarea_input_from_key(key));
    }

    pub(super) fn input_insert_text(&mut self, text: String) {
        self.reset_esc_chord();
        if !self.rewind_mode() {
            self.reset_input_history_navigation();
        }
        let _ = self.input.insert_str(text);
    }
}
