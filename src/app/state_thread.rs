//! Replace or fork transcript state while preserving the background summary service.

use super::input_history_state::InputHistoryState;
use super::render_cache_state::RenderCacheState;
use super::state::AppState;
use super::viewport_state::ViewportState;

impl AppState {
    pub(super) fn replace_transcript_from(&mut self, replacement: AppState) {
        self.summaries.replace_bindings(replacement.summaries);
        self.messages = replacement.messages;
        self.item_to_message_index = replacement.item_to_message_index;
        self.completed_item_ids = replacement.completed_item_ids;
        self.turn_status_by_id = replacement.turn_status_by_id;
        self.turn_diff_to_index = replacement.turn_diff_to_index;
        self.command_render_overrides = replacement.command_render_overrides;
        self.input_history = replacement.input_history;
        self.render_cache = RenderCacheState::new();
        self.viewport = ViewportState::new();
        self.turn_start_message_idx = self.active_turn_id.as_ref().map(|_| self.messages.len());
        self.mark_transcript_dirty();
    }

    pub(super) fn reset_for_forked_thread(&mut self, thread_id: impl Into<String>) {
        self.set_thread_id(thread_id);
        self.active_turn_id = None;
        self.messages.clear();
        self.render_cache = RenderCacheState::new();
        self.item_to_message_index.clear();
        self.completed_item_ids.clear();
        self.turn_status_by_id.clear();
        self.turn_diff_to_index.clear();
        self.command_render_overrides.clear();
        self.summaries.clear();
        self.input_history = InputHistoryState::new();
        self.turn_start_message_idx = None;
        self.viewport = ViewportState::new();
        self.context_usage = None;
        self.clear_pending_approval();
    }
}
