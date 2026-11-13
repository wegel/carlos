//! Fence a terminal Codex transport failure without losing the person's draft.

use super::models::Role;
use super::state::AppState;
use crate::backend::PromptOrigin;

/// Keep human input recoverable and discard automatic work after terminal closure.
pub(super) fn reject_closed_prompt(app: &mut AppState, text: &str, origin: PromptOrigin) -> bool {
    if !app.session.codex_scope().closed() {
        return false;
    }
    app.session.ralph_stop_after_failure();
    if origin == PromptOrigin::Human {
        app.set_input_text(text);
        app.append_message(
            Role::System,
            "Could not send message: Codex is closed; F5 reconnects; draft kept",
        );
    }
    app.set_hint("Codex is closed; F5 reconnects; draft kept");
    true
}

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
