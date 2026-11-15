//! AppState impl: text input editing, escape-chord, history navigation, and rewind mode.

use std::time::Instant;

use super::render::{compute_input_layout, textarea_input_from_key};

use super::state::AppState;
use super::view_models::TerminalSize;
use super::MSG_TOP;

impl AppState {
    // --- Input handling ---

    pub(super) fn input_is_empty(&self) -> bool {
        self.view.input.is_empty()
    }

    pub(super) fn input_text(&self) -> String {
        self.view.input.lines().join("\n")
    }

    pub(super) fn clear_input(&mut self) {
        self.view.input = super::input::make_input_area();
        self.reset_input_history_navigation();
    }

    pub(super) fn set_input_text(&mut self, text: &str) {
        self.view.input = super::input::make_input_area();
        if !text.is_empty() {
            let _ = self.view.input.insert_str(text);
        }
    }

    pub(super) fn reset_input_history_navigation(&mut self) {
        self.view.input_history.reset_navigation();
    }

    pub(super) fn reset_esc_chord(&mut self) {
        self.view.reset_esc_chord();
    }

    /// Preserve the embedded caller until the terminal runtime is removed.
    pub(super) fn suppress_esc_chord_after_interrupt(&mut self, now: Instant) {
        self.view.suppress_esc_chord_after_interrupt(now);
    }

    pub(super) fn expire_esc_chord(&mut self, now: Instant) {
        self.view.expire_esc_chord(now);
    }

    pub(super) fn register_escape_press(&mut self, now: Instant) -> bool {
        self.view.register_escape_press(now)
    }

    // --- Rewind ---

    pub(super) fn enter_rewind_mode(&mut self) {
        if !self.view.input_history.enter_rewind_mode(self.input_text()) {
            return;
        }
        self.view.viewport.auto_follow_bottom = false;
        let _ = self.navigate_input_history_up();
        if !self.session.claude_history().unknown().is_empty() {
            self.set_hint("Edit human prompt; F4 selects older prompts with unknown source");
        }
    }

    pub(super) fn exit_rewind_mode_restore(&mut self) {
        let Some(draft) = self.view.input_history.exit_rewind_mode_restore() else {
            return;
        };
        self.view.viewport.auto_follow_bottom = true;
        self.view.claude_unknown_selection = None;
        if !super::claude_edit::waiting(self) {
            self.view.claude_edit.failed = false;
        }
        self.set_input_text(&draft);
    }

    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.view.input_history.clear_rewind_mode_state();
        if self.view.claude_edit.failed {
            self.view.claude_edit.failed = false;
        }
        self.view.claude_unknown_selection = None;
        self.view.viewport.auto_follow_bottom = true;
    }

    pub(super) fn mark_user_turn_submitted(&mut self) {
        self.session.ralph_mark_user_turn_submitted();
    }

    pub(super) fn push_input_history(&mut self, text: &str) {
        self.record_prompt_history(text, None);
    }

    fn record_prompt_history(
        &mut self,
        text: &str,
        entry: Option<carlos_core::session::entry_id::EntryId>,
    ) {
        if self.session.record_prompt_history(text, entry) {
            self.reset_input_history_navigation();
        }
    }

    #[cfg(test)]
    pub(super) fn rewind_selected_turn_id(&self) -> Option<&str> {
        self.view
            .claude_unknown_selection
            .and_then(|index| self.session.claude_history().unknown().get(index))
            .map(|prompt| prompt.uuid())
            .or_else(|| {
                self.session
                    .input_history()
                    .turn_id(self.view.input_history.rewind_selected_entry_id()?)
            })
    }

    pub(super) fn navigate_input_history_up(&mut self) -> bool {
        if super::claude_rewind::navigate(self, true) {
            return true;
        }
        let Some(text) = self
            .view
            .input_history
            .navigate_up(self.session.input_history(), self.input_text())
        else {
            return false;
        };
        self.set_input_text(&text);
        true
    }

    pub(super) fn navigate_input_history_down(&mut self) -> bool {
        if super::claude_rewind::navigate(self, false) {
            return true;
        }
        let Some(text) = self
            .view
            .input_history
            .navigate_down(self.session.input_history())
        else {
            return false;
        };
        self.set_input_text(&text);
        true
    }

    pub(super) fn rewind_selected_message_idx(&self) -> Option<usize> {
        self.view
            .claude_unknown_selection
            .and_then(|index| self.session.claude_history().unknown().get(index))
            .and_then(|prompt| self.session.messages().position(prompt.entry_id()))
            .or_else(|| {
                self.view
                    .input_history
                    .rewind_selected_entry_id()
                    .and_then(|id| self.session.messages().position(id))
            })
    }

    pub(super) fn rewind_prompt_id(&self) -> Option<carlos_core::session::entry_id::EntryId> {
        self.view
            .claude_unknown_selection
            .and_then(|index| self.session.claude_history().unknown().get(index))
            .map(|prompt| prompt.entry_id())
            .or_else(|| self.view.input_history.rewind_prompt_id())
    }

    #[cfg(test)]
    pub(super) fn submitted_turn_count(&self) -> usize {
        self.session.input_history().history_len()
    }

    pub(super) fn align_rewind_scroll_to_selected_prompt(&mut self, size: TerminalSize) {
        if !self.rewind_mode()
            || (self.session.claude_edit_progress().is_some() || self.view.claude_edit.failed)
        {
            return;
        }
        let Some(msg_idx) = self.rewind_selected_message_idx() else {
            return;
        };
        if self.session.messages().is_empty() {
            return;
        }
        let width = self.transcript_width(size);
        if width == 0 {
            return;
        }
        let upto = msg_idx.min(self.session.messages().len().saturating_sub(1));
        self.view.layout.update_size(size);
        self.ensure_rendered_lines(width, Some(msg_idx));
        let rendered_upto = self
            .view
            .render_cache
            .block_start(upto)
            .expect("rendered entry");
        if rendered_upto == 0 {
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
        let target_line = rendered_upto.saturating_sub(1);
        self.view.viewport.scroll_top = target_line.saturating_sub(msg_height.saturating_sub(1));
    }

    pub(super) fn input_apply_key(&mut self, key: crossterm::event::KeyEvent) {
        self.reset_esc_chord();
        if !self.rewind_mode() {
            self.reset_input_history_navigation();
        }
        let _ = self.view.input.input(textarea_input_from_key(key));
    }

    pub(super) fn input_insert_text(&mut self, text: String) {
        self.reset_esc_chord();
        if !self.rewind_mode() {
            self.reset_input_history_navigation();
        }
        let _ = self.view.input.insert_str(text);
    }
}
