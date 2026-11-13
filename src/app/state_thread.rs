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
        self.reset_thread(thread_id, false);
    }

    /// Clear native transcript rows while process callbacks remain live.
    #[cfg(test)]
    pub(super) fn reset_preserving_approvals(&mut self, thread_id: impl Into<String>) {
        self.reset_thread(thread_id, true);
    }

    #[cfg(test)]
    fn reset_thread(&mut self, thread_id: impl Into<String>, preserve: bool) {
        self.set_thread_id(thread_id);
        self.session.reset_for_forked_thread();
        self.consume_core_events();
        self.view.render_cache = RenderCacheState::new();
        self.session.reset_prompt_history();
        self.view.input_history = InputHistoryView::default();
        self.view.viewport = ViewportState::new();
        if preserve {
            return;
        }
        if self.session.codex_scope().enabled() {
            super::codex_callbacks::reset(self);
        } else {
            self.clear_pending_approval();
        }
    }
}

#[cfg(test)]
impl AppState {
    pub(super) fn replace_transcript_for_test(&mut self, replacement: AppState) {
        self.session.clear_summaries();
        self.session.transcript_adopt_unkeyed(
            replacement
                .session
                .messages()
                .entries()
                .iter()
                .cloned()
                .map(Into::into)
                .collect(),
            carlos_core::session::history_identity::HistoryMode::SameThread,
        );
        self.view.render_cache = RenderCacheState::new();
        self.view.viewport = ViewportState::new();
        self.mark_transcript_dirty();
    }
}
