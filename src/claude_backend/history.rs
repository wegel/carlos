//! Claude CLI history import, parsing, and record construction.

// --- Imports ---

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};
use serde_json::{json, Value};

use super::exit_plan::{
    claude_exit_plan_approval_from_tool_call, claude_exit_plan_request_line,
    fallback_tool_result_item, synthetic_tool_result_item,
};
use super::session_file::{
    active_branch_uuids, find_session_file_for_resume, read_session_records_from_file,
    user_record_text,
};
use super::session_fork::fork_claude_session_from_projects_root;
use super::token_usage::{claude_context_usage_from_record, ClaudeContextUsage};
use super::types::{
    claude_projects_root, should_hide_claude_tool_transcript, ClaudeExitPlanApproval,
    ClaudeLaunchMode, ClaudeToolCall,
};

// --- Public types ---

#[derive(Debug, Clone)]
pub(crate) struct ClaudeLocalHistory {
    pub(crate) session_id: String,
    pub(crate) thread: Value,
    pub(crate) imported_item_count: usize,
    pub(crate) context_usage: Option<ClaudeContextUsage>,
    pub(crate) pending_approval_request: Option<String>,
}

// --- Record builders ---

fn user_message_item(text: &str) -> Value {
    json!({
        "type": "userMessage",
        "content": [{
            "type": "text",
            "text": text,
        }]
    })
}

fn agent_message_item(text: &str) -> Value {
    json!({
        "type": "agentMessage",
        "text": text,
    })
}

fn tool_call_item(tool_use_id: &str, tool_call: &ClaudeToolCall) -> Value {
    json!({
        "id": tool_use_id,
        "type": "toolCall",
        "tool": tool_call.name,
        "name": tool_call.name,
        "input": tool_call.input,
    })
}

// --- History record parsing ---

fn append_assistant_history_record(
    record: &Value,
    pending_tool_calls: &mut HashMap<String, ClaudeToolCall>,
    items: &mut Vec<Value>,
    pending_exit_plan_approval: &mut Option<ClaudeExitPlanApproval>,
) {
    let Some(message) = record.get("message").and_then(Value::as_object) else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };

    for part in content {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = part.get("text").and_then(Value::as_str) {
                    if !text.trim().is_empty() {
                        *pending_exit_plan_approval = None;
                        items.push(agent_message_item(text));
                    }
                }
            }
            Some("tool_use") => {
                *pending_exit_plan_approval = None;
                let tool_use_id = part
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        format!("claude-history-tool-{}", pending_tool_calls.len() + 1)
                    });
                let tool_call = ClaudeToolCall {
                    name: part
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("Tool")
                        .to_string(),
                    input: part.get("input").cloned().unwrap_or_else(|| json!({})),
                };
                if !should_hide_claude_tool_transcript(&tool_call.name, &tool_call.input) {
                    items.push(tool_call_item(&tool_use_id, &tool_call));
                }
                pending_tool_calls.insert(tool_use_id, tool_call);
            }
            _ => {}
        }
    }
}

fn append_user_history_record(
    record: &Value,
    pending_tool_calls: &mut HashMap<String, ClaudeToolCall>,
    items: &mut Vec<Value>,
    pending_exit_plan_approval: &mut Option<ClaudeExitPlanApproval>,
) {
    let Some(message) = record.get("message").and_then(Value::as_object) else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return;
    }

    if let Some(text) = user_record_text(record) {
        *pending_exit_plan_approval = None;
        items.push(user_message_item(&text));
    }

    let Some(parts) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    let tool_use_result = record
        .get("toolUseResult")
        .or_else(|| record.get("tool_use_result"));
    let mut saw_valid_tool_result = false;
    let mut exit_plan_approval_in_record = None;
    for part in parts {
        if part.get("type").and_then(Value::as_str) != Some("tool_result") {
            continue;
        }
        let tool_use_id = part
            .get("tool_use_id")
            .and_then(Value::as_str)
            .unwrap_or("");
        if tool_use_id.is_empty() {
            continue;
        }
        saw_valid_tool_result = true;

        let (item, exit_plan_approval) = if let Some(tool_call) =
            pending_tool_calls.remove(tool_use_id)
        {
            let hide_transcript =
                should_hide_claude_tool_transcript(&tool_call.name, &tool_call.input);
            let approval =
                claude_exit_plan_approval_from_tool_call(&tool_call, tool_use_id, part);
            let item = if hide_transcript {
                None
            } else {
                synthetic_tool_result_item(
                    &tool_call,
                    tool_use_id,
                    part,
                    tool_use_result,
                )
            };
            (item, approval)
        } else {
            (fallback_tool_result_item(tool_use_id, part), None)
        };
        if let Some(item) = item {
            items.push(item);
        }
        if let Some(approval) = exit_plan_approval {
            exit_plan_approval_in_record = Some(approval);
        }
    }
    if let Some(approval) = exit_plan_approval_in_record {
        *pending_exit_plan_approval = Some(approval);
    } else if saw_valid_tool_result {
        *pending_exit_plan_approval = None;
    }
}

fn parse_local_history_from_file(path: &Path, session_id: &str) -> Result<ClaudeLocalHistory> {
    let mut items = Vec::new();
    let mut pending_tool_calls = HashMap::new();
    let mut pending_exit_plan_approval = None;
    let mut context_usage = None;
    let records = read_session_records_from_file(path)?;

    let active_branch_uuids = active_branch_uuids(&records);
    for session_record in records {
        if session_record.is_sidechain {
            continue;
        }
        if let (Some(active_branch_uuids), Some(uuid)) =
            (active_branch_uuids.as_ref(), session_record.uuid.as_deref())
        {
            if !active_branch_uuids.contains(uuid) {
                continue;
            }
        }
        if let Some(usage) = claude_context_usage_from_record(&session_record.record) {
            context_usage = Some(usage);
        }
        match session_record.record.get("type").and_then(Value::as_str) {
            Some("assistant") => append_assistant_history_record(
                &session_record.record,
                &mut pending_tool_calls,
                &mut items,
                &mut pending_exit_plan_approval,
            ),
            Some("user") => append_user_history_record(
                &session_record.record,
                &mut pending_tool_calls,
                &mut items,
                &mut pending_exit_plan_approval,
            ),
            _ => {}
        }
    }

    let imported_item_count = items.len();
    Ok(ClaudeLocalHistory {
        session_id: session_id.to_string(),
        thread: json!({
            "id": session_id,
            "turns": [{
                "items": items,
            }]
        }),
        imported_item_count,
        context_usage,
        pending_approval_request: pending_exit_plan_approval
            .as_ref()
            .map(claude_exit_plan_request_line),
    })
}

pub(crate) fn fork_claude_local_history_from_projects_root(
    projects_root: &Path,
    cwd: &Path,
    source_session_id: &str,
    keep_turns: usize,
) -> Result<ClaudeLocalHistory> {
    let forked = fork_claude_session_from_projects_root(
        projects_root,
        cwd,
        source_session_id,
        keep_turns,
    )?;
    parse_local_history_from_file(&forked.path, &forked.session_id)
}

// --- Public loading API ---

pub(crate) fn load_claude_local_history_from_projects_root(
    projects_root: &Path,
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
) -> Result<Option<ClaudeLocalHistory>> {
    let session_path = match launch_mode {
        ClaudeLaunchMode::New => return Ok(None),
        ClaudeLaunchMode::Resume(session_id) => {
            find_session_file_for_resume(projects_root, cwd, session_id)
                .map(|path| (session_id.clone(), path))
        }
        // `claude --continue` does not expose its chosen resumed session up front, so
        // preloading local history here risks showing the wrong transcript before the live
        // backend confirms which session it actually continued.
        ClaudeLaunchMode::Continue => return Ok(None),
    };

    let Some((session_id, path)) = session_path else {
        return Ok(None);
    };
    match parse_local_history_from_file(&path, &session_id) {
        Ok(history) => Ok(Some(history)),
        Err(_) => Ok(None),
    }
}

pub(crate) fn load_claude_local_history(
    cwd: &Path,
    launch_mode: &ClaudeLaunchMode,
) -> Result<Option<ClaudeLocalHistory>> {
    let Some(projects_root) = claude_projects_root() else {
        return Ok(None);
    };
    if !projects_root.is_dir() {
        return Ok(None);
    }
    load_claude_local_history_from_projects_root(&projects_root, cwd, launch_mode)
}

pub(crate) fn fork_claude_local_history(
    cwd: &Path,
    source_session_id: &str,
    keep_turns: usize,
) -> Result<ClaudeLocalHistory> {
    let Some(projects_root) = claude_projects_root() else {
        bail!("Claude config directory is unavailable");
    };
    if !projects_root.is_dir() {
        bail!("Claude projects directory does not exist");
    }
    fork_claude_local_history_from_projects_root(
        &projects_root,
        cwd,
        source_session_id,
        keep_turns,
    )
}
