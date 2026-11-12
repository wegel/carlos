//! Keep local notices in the view without changing backend status.

use super::state::AppState;

impl AppState {
    /// Show a notice from this terminal without changing session facts.
    pub(super) fn set_hint(&mut self, text: impl Into<String>) {
        self.view.status_hint = Some(text.into());
    }
}
