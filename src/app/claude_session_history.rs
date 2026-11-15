//! Hydrates paged SDK history without turning automatic or unknown records into human prompts.

use carlos_core::session::models::Role;

use super::state::AppState;

/// Start the new helper generation before accepting its callbacks or metadata.
#[cfg(test)]
pub(super) fn switch_session(app: &mut AppState, generation: u64, id: &str) {
    let mut update = Default::default();
    app.session
        .switch_claude_session(generation, id, &mut update);
    let _ = app.apply_claude_update(update);
}

/// Report whether the user can explicitly recover a retained draft in any helper phase.
pub(super) fn has_unsent(app: &AppState) -> bool {
    !app.view.claude_edit.unsent_drafts.is_empty()
}

/// Load one retained draft only when doing so cannot replace the user's composer.
pub(super) fn load_unsent(app: &mut AppState) {
    if !app.input_is_empty() {
        app.append_message(
            Role::System,
            "Copy or clear the current draft before pressing F5".to_string(),
        );
        return;
    }
    if let Some(text) = app.view.claude_edit.unsent_drafts.pop_front() {
        app.set_input_text(&text);
    }
}
