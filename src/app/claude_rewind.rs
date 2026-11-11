//! Lets users explicitly branch before legacy Claude prompts whose source remains unknown.

use super::state::AppState;

/// Toggle the explicit unknown-source branch selector while rewind is open.
pub(super) fn toggle_unknown(app: &mut AppState) {
    if app.claude.history.unknown_selection.take().is_some() {
        app.reset_input_history_navigation();
        if !app.navigate_input_history_up() {
            app.clear_input();
        }
        app.set_status("Edit human prompt; F4 selects older prompts with unknown source");
        return;
    }
    let Some(index) = app.claude.history.unknown.len().checked_sub(1) else {
        app.set_status("No older prompts with unknown source");
        return;
    };
    select(app, index);
}

/// Move only within the explicitly selected legacy records.
pub(super) fn navigate(app: &mut AppState, up: bool) -> bool {
    let Some(index) = app.claude.history.unknown_selection else {
        return false;
    };
    let next = if up {
        index.saturating_sub(1)
    } else {
        (index + 1).min(app.claude.history.unknown.len() - 1)
    };
    select(app, next);
    true
}

fn select(app: &mut AppState, index: usize) {
    app.claude.history.unknown_selection = Some(index);
    let text = app.claude.history.unknown[index].text.clone();
    app.set_input_text(&text);
    app.set_status(
        "Source unknown: Enter forks before this prompt; F4 human prompts; Esc restores draft",
    );
}
