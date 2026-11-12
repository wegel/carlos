//! Apply session prompt changes to the local transcript caches and history cursor.

use super::state::AppState;

/// Keep the client's queue versus steer choice separate from accepted native inputs.
#[derive(Default)]
pub(super) struct ClaudePrompts {
    pub(super) queue: bool,
}

/// Associate a local echo with its native UUID before the next SDK frame is handled.
#[cfg(test)]
pub(super) fn attach_echo(app: &mut AppState, id: &str, kind: Option<&str>) {
    let key = app.item_key(&format!("claude-prompt:{id}"));
    let update = app.session.attach_claude_prompt_echo(id, &key, kind);
    app.apply_session_update(update);
}

/// Apply only a captured human steer classification.
pub(super) fn set_kind(app: &mut AppState, index: usize, kind: Option<&str>) {
    let update = app.session.classify_claude_prompt(index, kind);
    app.apply_session_update(update);
}

/// Preserve the redraw calls and navigation reset for each consumed native input.
pub(super) fn consume(app: &mut AppState, ids: &[String]) {
    for id in ids {
        let key = app.item_key(&format!("claude-prompt:{id}"));
        let update = app.session.consume_claude_prompt(id, &key);
        app.apply_session_update(update.appended);
        app.apply_session_update(update.classified);
        if update.reset_navigation {
            app.reset_input_history_navigation();
        }
    }
}

/// Preserve accepted queued inputs when native Claude clears consumed conversation work.
pub(super) fn reset(app: &mut AppState, clear: Option<&str>) {
    app.session.reset_claude_prompts(clear);
}
