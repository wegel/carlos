//! Attach native fixture echoes until the remaining session-state tests move to Core.

#[cfg(test)]
use super::state::AppState;

/// Associate a local echo with its native UUID before the next SDK frame is handled.
#[cfg(test)]
pub(super) fn attach_echo(app: &mut AppState, id: &str, kind: Option<&str>) {
    let key = app.item_key(&format!("claude-prompt:{id}"));
    app.session.attach_claude_prompt_echo(id, &key, kind);
    app.consume_core_events();
}
