//! Fence a terminal Codex transport failure without losing the person's draft.

use super::state::AppState;

impl AppState {
    /// Apply only the local controls and recovery text named by native closure.
    pub(super) fn apply_codex_closed(
        &mut self,
        closed: crate::session::codex::lifecycle::CodexClosed,
    ) {
        self.view.quota_next_draw = None;
        super::codex_settings::apply(self, closed.settings);
        super::codex_queue_receive::recover(self, closed.recovery);
        self.view.codex_goals.confirm = None;
        self.view.codex_goals.error = None;
        self.view.codex_callbacks = Default::default();
        self.drop_retired_approval_hints(closed.retired);
        self.apply_item_changes(closed.transcript);
        self.set_hint(closed.hint);
    }
}
