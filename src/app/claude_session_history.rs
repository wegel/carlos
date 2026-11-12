//! Hydrates paged SDK history without turning automatic or unknown records into human prompts.

use std::collections::VecDeque;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::claude_backend::FactTransfer;

use super::claude_state::{ClaudePhase, ClaudeUiState};
use super::models::Role;
use super::notifications::ServerRequestAction;
use super::state::AppState;
use crate::session::approval_state::ApprovalQueue;

/// Retains public native records and a separate list of legacy prompts with unknown authors.
#[derive(Default)]
pub(super) struct ClaudeHistory {
    pub(super) records: Vec<Value>,
    pub(super) unknown: Vec<UnknownPrompt>,
    pub(super) unknown_selection: Option<usize>,
    pub(super) edited_prompt: Option<String>,
    pub(super) edit: Option<super::claude_edit::EditState>,
    unsent_drafts: VecDeque<String>,
    pub(super) transfer: FactTransfer,
}

/// Describes an old main-thread prompt whose author Carlos cannot establish.
pub(super) struct UnknownPrompt {
    pub(super) uuid: String,
    pub(super) text: String,
    pub(super) entry_id: crate::session::entry_id::EntryId,
}

/// Start the new helper generation before accepting its callbacks or metadata.
pub(super) fn switch_session(app: &mut AppState, generation: u64, id: &str) {
    super::claude_edit::retire_queue(app);
    let unsent_drafts = std::mem::take(&mut app.claude.history.unsent_drafts);
    let edit = app.claude.history.edited_prompt.take();
    app.session.switch_claude_runtime_choices();
    app.reset_for_forked_thread(id);
    app.claude = ClaudeUiState::default();
    app.session.reset_claude_metadata();
    app.view.claude_quota_next_draw = None;
    let retired = app.session.clear_approval_queue(ApprovalQueue::Claude);
    app.drop_retired_approval_hints(retired);
    app.view.confirm_allow = None;
    app.claude.enabled = true;
    app.claude.generation = generation;
    app.session.set_claude_phase(ClaudePhase::Starting);
    app.claude.history.edit = edit
        .as_ref()
        .map(|_| super::claude_edit::EditState::Starting);
    app.claude.history.edited_prompt = edit;
    app.claude.history.unsent_drafts = unsent_drafts;
    app.set_available_models(Vec::new());
    app.set_status("Claude starts the edited branch");
}

/// Continue reading one stable history snapshot, then start the native query asynchronously.
pub(super) fn receive(
    app: &mut AppState,
    method: &str,
    result: &Value,
) -> Result<ServerRequestAction> {
    if method == "sessions.info" {
        if result["sessionId"].as_str() != Some(app.session.thread_id().as_str()) {
            bail!("The requested Claude session was not found");
        }
        app.set_thread_name(result["customTitle"].as_str());
        app.set_status("Reading Claude history");
        return Ok(control(
            "sessions.read",
            json!({"sessionId":app.session.thread_id()}),
        ));
    }
    if let Some(offset) = result["nextOffset"].as_u64() {
        return Ok(control(
            "sessions.read",
            json!({"sessionId":app.session.thread_id(),"offset":offset,
            "snapshotId":result["snapshotId"], "chunkOffset":result["chunkOffset"].as_u64().unwrap_or(0)}),
        ));
    }
    app.session.set_context_usage(None);
    Ok(control(
        "start",
        json!({"sessionId":app.session.thread_id()}),
    ))
}

/// Append an already decoded history prompt with its explicit source and native UUID.
pub(super) fn append_prompt(app: &mut AppState, facts: &Value) {
    let source = facts["source"].as_str().unwrap_or("unknown");
    let kind = facts["inputKind"].as_str();
    let uuid = facts["uuid"].as_str().unwrap_or("");
    let text = facts["text"].as_str().unwrap_or("").to_string();
    let key = format!("claude-prompt:{uuid}");
    if !uuid.is_empty() && app.has_item_mapping(&key) {
        return;
    }
    let start = app.session.messages().len();
    match source {
        "human" => {
            let index = app.append_message(Role::User, text.clone());
            super::claude_prompts::set_kind(app, index, kind);
            app.record_input_history(&text, Some(index));
            app.session
                .record_prompt_turn(app.session.messages()[index].id, uuid);
        }
        "unknown" => {
            let index = app.append_message(Role::System, format!("[Source unknown] {text}"));
            app.claude.history.unknown.push(UnknownPrompt {
                uuid: uuid.to_string(),
                text,
                entry_id: app.session.messages()[index].id,
            });
        }
        "automatic" => {
            app.append_message(Role::System, format!("Automatic prompt: {text}"));
        }
        _ => {}
    }
    if app.session.messages().len() > start && !uuid.is_empty() {
        app.put_item_mapping(&key, start);
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
    let Some(text) = app.claude.history.edited_prompt.take() else {
        return;
    };
    if app.input_is_empty() {
        app.set_input_text(&text);
    } else if app.input_text() != text {
        app.append_message(
            Role::System,
            format!("Unsent edited draft (F5 loads it when the composer is empty):\n{text}"),
        );
        app.claude.history.unsent_drafts.push_back(text);
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
    let drafts = app.session.ralph_take_queued_human_drafts();
    for text in drafts.iter().rev() {
        app.discard_pending_input_history(text);
    }
    for text in drafts {
        app.append_message(
            Role::System,
            format!("Unsent queued draft (F5 loads it when the composer is empty):\n{text}"),
        );
        app.claude.history.unsent_drafts.push_back(text);
    }
}

/// Report whether the user can explicitly recover a retained draft in any helper phase.
pub(super) fn has_unsent(app: &AppState) -> bool {
    !app.claude.history.unsent_drafts.is_empty()
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
    if let Some(text) = app.claude.history.unsent_drafts.pop_front() {
        app.set_input_text(&text);
    }
}
