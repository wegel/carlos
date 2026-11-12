//! AppState impl: text input editing, escape-chord, history navigation, and rewind mode.

use std::time::{Duration, Instant};

use super::render::{compute_input_layout, textarea_input_from_key};
use super::state::AppState;
use super::view_models::TerminalSize;
use super::MSG_TOP;

const ESC_CHORD_WINDOW: Duration = Duration::from_millis(700);

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
        self.view.esc_armed_at = None;
        self.view.esc_interrupted_at = None;
    }

    /// Require a quiet Escape interval after stopping work before accepting an idle chord.
    pub(super) fn suppress_esc_chord_after_interrupt(&mut self, now: Instant) {
        self.view.esc_armed_at = None;
        self.view.esc_interrupted_at = Some(now);
    }

    pub(super) fn expire_esc_chord(&mut self, now: Instant) {
        if let Some(armed_at) = self.view.esc_armed_at {
            if now.duration_since(armed_at) > ESC_CHORD_WINDOW {
                self.view.esc_armed_at = None;
            }
        }
    }

    pub(super) fn register_escape_press(&mut self, now: Instant) -> bool {
        if let Some(interrupted_at) = self.view.esc_interrupted_at.take() {
            if now.duration_since(interrupted_at) <= ESC_CHORD_WINDOW {
                self.suppress_esc_chord_after_interrupt(now);
                return false;
            }
        }
        if let Some(armed_at) = self.view.esc_armed_at {
            if now.duration_since(armed_at) <= ESC_CHORD_WINDOW {
                self.view.esc_armed_at = None;
                return true;
            }
        }
        self.view.esc_armed_at = Some(now);
        false
    }

    // --- Rewind ---

    pub(super) fn enter_rewind_mode(&mut self) {
        if !self.view.input_history.enter_rewind_mode(self.input_text()) {
            return;
        }
        self.view.viewport.auto_follow_bottom = false;
        let _ = self.navigate_input_history_up();
        if !self.claude.history.unknown.is_empty() {
            self.set_hint("Edit human prompt; F4 selects older prompts with unknown source");
        }
    }

    pub(super) fn exit_rewind_mode_restore(&mut self) {
        let Some(draft) = self.view.input_history.exit_rewind_mode_restore() else {
            return;
        };
        self.view.viewport.auto_follow_bottom = true;
        self.claude.history.unknown_selection = None;
        if !super::claude_edit::waiting(self) {
            self.claude.history.edit = None;
        }
        self.set_input_text(&draft);
    }

    pub(super) fn clear_rewind_mode_state(&mut self) {
        self.view.input_history.clear_rewind_mode_state();
        if matches!(
            self.claude.history.edit,
            Some(super::claude_edit::EditState::Failed)
        ) {
            self.claude.history.edit = None;
        }
        self.claude.history.unknown_selection = None;
        self.view.viewport.auto_follow_bottom = true;
    }

    #[cfg(test)]
    pub(super) fn rewind_fork_from_message_idx(&mut self, message_idx: Option<usize>) {
        let Some(idx) = message_idx else {
            return;
        };
        if idx > self.session.messages().len() {
            return;
        }

        let dirty_from = self.session.truncate_at(idx);
        self.view.viewport.selection = None;
        self.view.viewport.mouse_drag_mode = super::selection::MouseDragMode::Undecided;
        self.view.viewport.auto_follow_bottom = true;
        self.view.viewport.scroll_top = self
            .view
            .viewport
            .scroll_top
            .min(self.session.messages().len());
        self.session.retain_summary_items();
        self.session.retain_prompt_entries();
        self.view
            .input_history
            .refresh_selected_entry(self.session.input_history());
        self.mark_transcript_dirty_from(dirty_from);
    }

    pub(super) fn mark_user_turn_submitted(&mut self) {
        self.session.ralph_mark_user_turn_submitted();
    }

    pub(super) fn push_input_history(&mut self, text: &str) {
        self.record_prompt_history(text, None);
    }

    pub(super) fn discard_pending_input_history(&mut self, text: &str) {
        if self.session.discard_pending_prompt(text) {
            self.reset_input_history_navigation();
        }
    }

    pub(super) fn record_input_history(&mut self, text: &str, message_idx: Option<usize>) {
        self.record_prompt_history(
            text,
            message_idx
                .and_then(|index| self.session.messages().get(index).map(|message| message.id)),
        );
    }

    fn record_prompt_history(
        &mut self,
        text: &str,
        entry: Option<crate::session::entry_id::EntryId>,
    ) {
        if self.session.record_prompt_history(text, entry) {
            self.reset_input_history_navigation();
        }
    }

    pub(super) fn rewind_native_request(&self) -> Option<crate::backend::RewindUserMessage> {
        self.session.input_history().native_request(
            self.view.input_history.rewind_selected_history_index()?,
            self.view.input_history.rewind_selected_entry_id()?,
        )
    }

    pub(super) fn record_input_turn_id(&mut self, item_id: &str, turn_id: &str) {
        let previous = self.codex.item_turn.replace(turn_id.to_string());
        let message_idx = self.item_message_index(item_id);
        self.codex.item_turn = previous;
        if let Some(index) = message_idx {
            self.session
                .record_prompt_turn(self.session.messages()[index].id, turn_id);
        }
        let position = self.session.record_native_user(
            item_id,
            turn_id,
            message_idx.map(|index| self.session.messages()[index].id),
        );
        if let (Some(index), Some(position)) = (message_idx, position) {
            let kind = if position == 0 {
                super::models::MessageKind::Plain
            } else {
                super::models::MessageKind::UserSteer
            };
            if self.session.messages()[index].kind != kind {
                self.session.transcript_set_kind(index, kind);
                self.mark_transcript_dirty_from(index);
            }
        }
    }

    pub(super) fn rewind_selected_turn_id(&self) -> Option<&str> {
        self.claude
            .history
            .unknown_selection
            .and_then(|index| self.claude.history.unknown.get(index))
            .map(|prompt| prompt.uuid.as_str())
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
        self.claude
            .history
            .unknown_selection
            .and_then(|index| self.claude.history.unknown.get(index))
            .and_then(|prompt| self.session.messages().position(prompt.entry_id))
            .or_else(|| {
                self.view
                    .input_history
                    .rewind_selected_entry_id()
                    .and_then(|id| self.session.messages().position(id))
            })
    }

    pub(super) fn rewind_selected_history_index(&self) -> Option<usize> {
        self.claude
            .history
            .unknown_selection
            .map(|_| usize::MAX)
            .or_else(|| self.view.input_history.rewind_selected_history_index())
    }

    pub(super) fn submitted_turn_count(&self) -> usize {
        self.session.input_history().history_len()
    }

    pub(super) fn align_rewind_scroll_to_selected_prompt(&mut self, size: TerminalSize) {
        if !self.rewind_mode() || self.claude.history.edit.is_some() {
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
        let rendered_upto = self.view.render_cache.rendered_block_offsets[upto];
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
