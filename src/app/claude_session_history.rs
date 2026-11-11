//! Hydrates paged SDK history without turning automatic or unknown records into human prompts.

use std::collections::VecDeque;

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};

use crate::claude_backend::{translate_claude_line, ClaudeTranslationState};

use super::claude_state::{ClaudePhase, ClaudeUiState};
use super::models::Role;
use super::notifications::{handle_server_message_line, ServerRequestAction};
use super::state::AppState;

/// Retains public native records and a separate list of legacy prompts with unknown authors.
#[derive(Default)]
pub(super) struct ClaudeHistory {
    pub(super) records: Vec<Value>,
    pub(super) unknown: Vec<UnknownPrompt>,
    pub(super) unknown_selection: Option<usize>,
    pub(super) edited_prompt: Option<String>,
    pub(super) edit: Option<super::claude_edit::EditState>,
    unsent_drafts: VecDeque<String>,
    translation: ClaudeTranslationState,
    chunk: Vec<u8>,
}

/// Describes an old main-thread prompt whose author Carlos cannot establish.
pub(super) struct UnknownPrompt {
    pub(super) uuid: String,
    pub(super) text: String,
    pub(super) message_idx: usize,
}

/// Start the new helper generation before accepting its callbacks or metadata.
pub(super) fn switch_session(app: &mut AppState, generation: u64, id: &str) {
    super::claude_edit::retire_queue(app);
    let unsent_drafts = std::mem::take(&mut app.claude.history.unsent_drafts);
    let selection = std::mem::take(&mut app.claude.selection);
    let edit = app.claude.history.edited_prompt.take();
    if selection.inherits_model && app.runtime.pending_model.is_none() {
        app.runtime.current_model = None;
    }
    if selection.inherits_effort && app.runtime.pending_effort.is_none() {
        app.runtime.current_effort = None;
    }
    app.reset_for_forked_thread(id);
    app.claude = ClaudeUiState::default();
    app.claude.enabled = true;
    app.claude.selection = selection;
    app.claude.generation = generation;
    app.claude.phase = ClaudePhase::Starting;
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
        if result["sessionId"].as_str() != Some(app.thread_id.as_str()) {
            bail!("The requested Claude session was not found");
        }
        app.set_thread_name(result["customTitle"].as_str());
        app.set_status("Reading Claude history");
        return Ok(control("sessions.read", json!({"sessionId":app.thread_id})));
    }
    if let Some(chunk) = result.get("chunk") {
        read_chunk(app, chunk)?;
    } else {
        for record in result["messages"]
            .as_array()
            .context("SDK history page has no messages")?
        {
            append_record(app, record)?;
        }
    }
    if let Some(offset) = result["nextOffset"].as_u64() {
        return Ok(control(
            "sessions.read",
            json!({"sessionId":app.thread_id,"offset":offset,
            "snapshotId":result["snapshotId"], "chunkOffset":result["chunkOffset"].as_u64().unwrap_or(0)}),
        ));
    }
    app.context_usage = None;
    Ok(control("start", json!({"sessionId":app.thread_id})))
}

fn read_chunk(app: &mut AppState, chunk: &Value) -> Result<()> {
    let offset = chunk["offset"]
        .as_u64()
        .context("History chunk has no offset")? as usize;
    let total = chunk["totalBytes"]
        .as_u64()
        .context("History chunk has no size")? as usize;
    let bytes = STANDARD.decode(
        chunk["data"]
            .as_str()
            .context("History chunk has no data")?,
    )?;
    let buffer = &mut app.claude.history.chunk;
    if total > 16 * 1024 * 1024 || offset != buffer.len() || buffer.len() + bytes.len() > total {
        bail!("Claude history chunk exceeds its declared size or arrived out of order");
    }
    buffer.extend(bytes);
    if buffer.len() == total {
        let record = serde_json::from_slice(buffer)?;
        buffer.clear();
        append_record(app, &record)?;
    }
    Ok(())
}

fn append_record(app: &mut AppState, record: &Value) -> Result<()> {
    let message = &record["message"];
    if message["session_id"].as_str() != Some(app.thread_id.as_str()) {
        bail!("Claude history belongs to a different session");
    }
    app.claude.history.records.push(message.clone());
    if crate::claude_backend::is_child_message(message)
        || crate::claude_backend::is_child_report_notice(message)
    {
        return Ok(());
    }
    if message["type"] == "user" {
        if let Some(text) = prompt_text(&message["message"]["content"]) {
            append_prompt(
                app,
                record["source"].as_str().unwrap_or("unknown"),
                record["inputKind"].as_str(),
                message,
                text,
            );
        }
    }
    if message["type"] == "system" && message["subtype"] == "compact_boundary" {
        app.append_message(Role::System, "Conversation compacted".to_string());
    }
    if matches!(message["type"].as_str(), Some("user" | "assistant")) {
        let translation = &mut app.claude.history.translation;
        if message["type"] == "assistant" {
            translation.begin_history_snapshot();
        }
        let output = translate_claude_line(translation, &message.to_string())?;
        app.with_historical_tools(|app| -> Result<()> {
            for line in output.lines {
                let parsed: Value = serde_json::from_str(&line)?;
                if parsed["method"]
                    .as_str()
                    .is_some_and(|method| method.starts_with("item/"))
                {
                    let _ = handle_server_message_line(app, &line);
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn append_prompt(
    app: &mut AppState,
    source: &str,
    kind: Option<&str>,
    message: &Value,
    text: String,
) {
    let uuid = message["uuid"].as_str().unwrap_or("");
    match source {
        "human" => {
            let index = app.append_message(Role::User, text.clone());
            super::claude_prompts::set_kind(app, index, kind);
            app.record_input_history(&text, Some(index));
            app.input_history.record_turn_id(index, uuid);
        }
        "unknown" => {
            let index = app.append_message(Role::System, format!("[Source unknown] {text}"));
            app.claude.history.unknown.push(UnknownPrompt {
                uuid: uuid.to_string(),
                text,
                message_idx: index,
            });
        }
        "automatic" => {
            app.append_message(Role::System, format!("Automatic prompt: {text}"));
        }
        _ => {}
    }
}

fn prompt_text(content: &Value) -> Option<String> {
    let text = content.as_str().map(str::to_string).or_else(|| {
        content.as_array().map(|parts| {
            parts
                .iter()
                .filter(|part| part["type"] == "text")
                .filter_map(|part| part["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
    })?;
    (!text.trim().is_empty()).then_some(text)
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
    let drafts = app.ralph_runtime.take_queued_human_drafts();
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
