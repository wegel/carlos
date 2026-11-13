//! Hydrates paged SDK history without turning automatic or unknown records into human prompts.

use anyhow::Result;
use serde_json::Value;

use super::models::Role;
use super::state::AppState;
use crate::session::actions::ServerRequestAction;
use crate::session::approval_state::ApprovalQueue;

/// Start the new helper generation before accepting its callbacks or metadata.
pub(super) fn switch_session(app: &mut AppState, generation: u64, id: &str) {
    super::claude_edit::retire_queue(app);
    let unsent_drafts = std::mem::take(&mut app.view.claude_edit.unsent_drafts);
    let edit = take_edit_prompt(app);
    app.session.switch_claude_runtime_choices();
    app.reset_for_forked_thread(id);
    app.view.claude_answers = Default::default();
    app.session.reset_claude_for_switch(generation);
    app.view.claude_tasks = Default::default();
    app.view.claude_queue = false;
    app.view.claude_unknown_selection = None;
    app.view.claude_quota_next_draw = None;
    let retired = app.session.clear_approval_queue(ApprovalQueue::Claude);
    app.drop_retired_approval_hints(retired);
    app.view.confirm_allow = None;
    app.view.claude_edit = Default::default();
    app.session.resume_claude_edit(edit);
    app.view.claude_edit.unsent_drafts = unsent_drafts;
    app.set_available_models(Vec::new());
    app.set_status("Claude starts the edited branch");
}

/// Continue reading one stable history snapshot, then start the native query asynchronously.
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

fn control(method: &str, params: Value) -> ServerRequestAction {
    ServerRequestAction::ClaudeControl {
        method: method.to_string(),
        params,
    }
}

/// Keep an edited prompt available when its replacement helper cannot start.
pub(super) fn restore_edit(app: &mut AppState) {
    let Some(text) = take_edit_prompt(app) else {
        return;
    };
    if app.input_is_empty() {
        app.set_input_text(&text);
    } else if app.input_text() != text {
        app.append_message(
            Role::System,
            format!("Unsent edited draft (F5 loads it when the composer is empty):\n{text}"),
        );
        app.view.claude_edit.unsent_drafts.push_back(text);
    }
}

/// Keep queued human text recoverable while leaving a newer composer draft untouched.
pub(super) fn recover_queued(app: &mut AppState) {
    retain_queued(app);
    if app.input_is_empty() {
        load_unsent(app);
    }
}

/// Retain local human drafts for explicit recovery without filling the new branch's composer.
pub(super) fn retain_queued(app: &mut AppState) {
    let (drafts, reset_navigation) = app.session.retain_claude_queued_drafts();
    if reset_navigation {
        app.reset_input_history_navigation();
    }
    for text in drafts {
        app.append_message(
            Role::System,
            format!("Unsent queued draft (F5 loads it when the composer is empty):\n{text}"),
        );
        app.view.claude_edit.unsent_drafts.push_back(text);
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

/// Take submitted recovery text once for the queue or the local composer.
pub(super) fn take_edit_prompt(app: &mut AppState) -> Option<String> {
    app.session.take_claude_edit_prompt()
}
