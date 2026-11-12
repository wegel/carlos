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

    /// Apply a checked fork draft and its session-owned metadata.
    pub(super) fn replace_history_draft(
        &mut self,
        draft: crate::session::item_history_draft::HistoryDraft,
        mode: crate::session::history_identity::HistoryMode,
    ) {
        let changes = self.session.replace_native_history(draft, mode);
        self.reset_history_view();
        self.apply_item_changes(changes);
    }

    pub(super) fn reset_for_forked_thread(&mut self, thread_id: impl Into<String>) {
        self.reset_thread(thread_id, false);
    }

    /// Clear native transcript rows while process callbacks remain live.
    pub(super) fn reset_preserving_approvals(&mut self, thread_id: impl Into<String>) {
        self.reset_thread(thread_id, true);
    }

    fn reset_thread(&mut self, thread_id: impl Into<String>, preserve: bool) {
        self.set_thread_id(thread_id);
        let update = self.session.reset_for_forked_thread();
        if update.transcript_reset {
            self.view.render_cache = RenderCacheState::new();
        }
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
            crate::session::history_identity::HistoryMode::SameThread,
        );
        self.view.render_cache = RenderCacheState::new();
        self.view.viewport = ViewportState::new();
        self.mark_transcript_dirty();
    }
}
