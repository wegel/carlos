//! Restore queue input without overwriting a newer composer draft.

use super::state::AppState;

/// Restore an unsent input without replacing text the user typed while awaiting a reply.
pub(super) fn recover(app: &mut AppState, text: Option<String>) {
    if let Some(text) = text {
        if app.input_is_empty() {
            app.set_input_text(&text);
        } else {
            app.view.codex_queue.recovery = Some(text);
        }
    }
}
