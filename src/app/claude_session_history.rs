//! Hydrates paged SDK history without turning automatic or unknown records into human prompts.

#[cfg(test)]
use anyhow::Result;
#[cfg(test)]
use serde_json::Value;

use super::models::Role;
use super::state::AppState;
#[cfg(test)]
use crate::session::actions::ServerRequestAction;

/// Start the new helper generation before accepting its callbacks or metadata.
#[cfg(test)]
pub(super) fn switch_session(app: &mut AppState, generation: u64, id: &str) {
    let mut update = Default::default();
    app.session
        .switch_claude_session(generation, id, &mut update);
    let _ = app.apply_claude_update(update);
}

/// Continue reading one stable history snapshot, then start the native query asynchronously.
#[cfg(test)]
pub(super) fn receive(
    app: &mut AppState,
    method: &str,
    result: &Value,
) -> Result<ServerRequestAction> {
    let (next_method, params) = app.session.claude_history_control(method, result)?;
    if method == "sessions.info" {
        app.set_thread_name(result["customTitle"].as_str());
        app.set_status("Reading Claude history");
    }
    Ok(control(next_method, params))
}

/// Append a classified history prompt and apply its original redraw and cursor effects.
#[cfg(test)]
pub(super) fn append_prompt(app: &mut AppState, facts: &Value) {
    let uuid = facts["uuid"].as_str().unwrap_or("");
    let key = app.item_key(&format!("claude-prompt:{uuid}"));
    let update = app.session.append_claude_history_prompt(facts, &key);
    app.apply_session_update(update.appended);
    app.apply_session_update(update.classified);
    if update.reset_navigation {
        app.reset_input_history_navigation();
    }
}

#[cfg(test)]
fn control(method: &str, params: Value) -> ServerRequestAction {
    ServerRequestAction::ClaudeControl {
        method: method.to_string(),
        params,
    }
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
