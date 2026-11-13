//! Apply only redraw reports from the session's automatic-work guards.

use anyhow::Result;

use super::state::AppState;

impl AppState {
    /// Request a user toggle through the native-goal and automatic-work owner.
    pub(super) fn request_ralph_toggle(&mut self) -> Result<()> {
        let changes = Default::default();
        let result = self.session.request_ralph_toggle();
        self.apply_item_changes(changes);
        result
    }
    /// Apply a deferred toggle only after the session checks native goal ownership.
    #[cfg(test)]
    pub(super) fn apply_pending_ralph_toggle(&mut self) -> Result<()> {
        let changes = Default::default();
        let result = self.session.apply_pending_ralph_toggle();
        self.apply_item_changes(changes);
        result
    }
}
