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
}
