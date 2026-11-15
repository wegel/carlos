//! Replace or fork transcript state while preserving the background summary service.

use super::input_history_state::InputHistoryView;
use super::render_cache_state::RenderCacheState;
use super::state::AppState;
use super::viewport_state::ViewportState;

impl AppState {
    /// Reset local history navigation and viewport after native history is adopted.
    pub(super) fn reset_history_view(&mut self) {
        self.view.input_history = InputHistoryView::default();
        self.view.render_cache = RenderCacheState::new();
        self.view.viewport = ViewportState::new();
    }

    #[cfg(test)]
    pub(super) fn reset_for_forked_thread(&mut self, thread_id: impl Into<String>) {
        self.reset_thread(thread_id);
    }

    #[cfg(test)]
    fn reset_thread(&mut self, thread_id: impl Into<String>) {
        self.set_thread_id(thread_id);
        self.session.reset_for_forked_thread();
        self.consume_core_events();
        self.view.render_cache = RenderCacheState::new();
        self.session.reset_prompt_history();
        self.view.input_history = InputHistoryView::default();
        self.view.viewport = ViewportState::new();
        if self.session.codex_scope().enabled() {
            super::codex_callbacks::reset(self);
        } else {
            self.clear_pending_approval();
        }
    }
}
