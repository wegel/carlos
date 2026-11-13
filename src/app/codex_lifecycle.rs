//! Fence a terminal Codex transport failure without losing the person's draft.

use serde_json::{Map, Value};

use super::models::Role;
use super::state::AppState;
use crate::backend::PromptOrigin;

/// Close the UI once; later frames cannot reopen a turn or an approval.
pub(super) fn close(app: &mut AppState, params: &Map<String, Value>) {
    close_with_guidance(app, params, "");
}

/// Keep retry keys in the view while recording the backend failure as a fact.
pub(super) fn close_with_guidance(app: &mut AppState, params: &Map<String, Value>, guidance: &str) {
    let Some(closed) = app.session.close_codex_native(params, guidance) else {
        return;
    };
    app.view.quota_next_draw = None;
    super::codex_settings::apply(app, closed.settings);
    super::codex_queue_receive::recover(app, closed.recovery);
    app.view.codex_goals.confirm = None;
    app.view.codex_goals.error = None;
    app.view.codex_callbacks = Default::default();
    app.drop_retired_approval_hints(closed.retired);
    app.apply_item_changes(closed.transcript);
    app.set_hint(closed.hint);
}

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
