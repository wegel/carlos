//! Exit plan approval, tool result synthesis, and shared protocol helpers.

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};

use super::types::{
    ClaudeAllowedPrompt, ClaudeExitPlanApproval, ClaudeToolCall,
    CLAUDE_EXIT_PLAN_FALLBACK_TEXT, CLAUDE_EXIT_PLAN_REQUEST_METHOD,
};

// --- Exit plan approval ---

pub(super) fn claude_exit_plan_request_id(tool_use_id: &str) -> Value {
    json!({
        "backend": "claude",
        "kind": "exitPlanMode",
        "toolUseId": tool_use_id,
    })
}

pub(super) fn claude_exit_plan_approval_from_tool_call(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
) -> Option<ClaudeExitPlanApproval> {
    if tool_call.name != "ExitPlanMode" {
        return None;
    }

    let is_error = part
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !is_error {
        return None;
    }

    let content_text = value_to_string(part.get("content")?);
    if !content_text.contains(CLAUDE_EXIT_PLAN_FALLBACK_TEXT) {
        return None;
    }

    let plan = tool_call
        .input
        .get("plan")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let plan_file_path = tool_call
        .input
        .get("planFilePath")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    let allowed_prompts = parse_allowed_prompts(&tool_call.input);

    Some(ClaudeExitPlanApproval {
        tool_use_id: tool_use_id.to_string(),
        plan,
        plan_file_path,
        allowed_prompts,
    })
}

fn parse_allowed_prompts(input: &Value) -> Vec<ClaudeAllowedPrompt> {
    let Some(entries) = input.get("allowedPrompts").and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let obj = entry.as_object()?;
            let prompt = obj
                .get("prompt")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())?
                .to_string();
            let tool = obj
                .get("tool")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned);
            Some(ClaudeAllowedPrompt { prompt, tool })
        })
        .collect()
}

pub(super) fn claude_exit_plan_request_line(approval: &ClaudeExitPlanApproval) -> String {
    let allowed_prompts = approval
        .allowed_prompts
        .iter()
        .map(|prompt| {
            json!({
                "prompt": prompt.prompt,
                "tool": prompt.tool,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "jsonrpc": "2.0",
        "id": claude_exit_plan_request_id(&approval.tool_use_id),
        "method": CLAUDE_EXIT_PLAN_REQUEST_METHOD,
        "params": {
            "toolUseId": approval.tool_use_id,
            "plan": approval.plan,
            "planFilePath": approval.plan_file_path,
            "allowedPrompts": allowed_prompts,
        }
    })
    .to_string()
}

pub(crate) fn claude_approval_follow_up_text<'a>(
    request_id: &Value,
    result: &Value,
) -> Result<Option<&'a str>> {
    let backend = request_id
        .get("backend")
        .and_then(Value::as_str)
        .unwrap_or("");
    let kind = request_id.get("kind").and_then(Value::as_str).unwrap_or("");

    if backend != "claude" || kind != "exitPlanMode" {
        return Ok(None);
    }

    let _tool_use_id = request_id
        .get("toolUseId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .context("missing Claude approval toolUseId")?;
    let decision = result
        .get("decision")
        .and_then(Value::as_str)
        .context("missing Claude approval decision")?;
    let follow_up = match decision {
        "accept" => "The plan is approved. Continue with the planned implementation now.",
        "decline" => {
            "Do not exit plan mode yet. Stay in plan mode, revise the plan, and then present an updated plan for approval."
        }
        "cancel" => "Cancel the exit from plan mode and stay in plan mode.",
        other => bail!("unsupported Claude approval decision: {other}"),
    };
    Ok(Some(follow_up))
}

// --- Tool result synthesis ---

pub(super) fn synthetic_tool_result_item(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
    tool_use_result: Option<&Value>,
) -> Option<Value> {
    let lower = tool_call.name.to_ascii_lowercase();
    let is_error = part
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let content = part.get("content")?;
    let content_text = value_to_string(content);

    if lower == "bash" {
        return Some(synthetic_bash_result(
            tool_call, tool_use_id, is_error, &content_text, tool_use_result,
        ));
    }
    if !is_error && (lower == "read" || (lower != "write" && lower != "edit")) {
        return None;
    }
    Some(synthetic_file_tool_result(
        tool_call, tool_use_id, is_error, &content_text, tool_use_result,
    ))
}

fn synthetic_bash_result(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    is_error: bool,
    content_text: &str,
    tool_use_result: Option<&Value>,
) -> Value {
    let command = tool_call
        .input
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (stdout, stderr, interrupted) = tool_use_result
        .and_then(Value::as_object)
        .map(|obj| {
            let stdout = obj.get("stdout").and_then(Value::as_str).unwrap_or("");
            let stderr = obj.get("stderr").and_then(Value::as_str).unwrap_or("");
            let interrupted = obj
                .get("interrupted")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            (stdout.to_string(), stderr.to_string(), interrupted)
        })
        .unwrap_or_else(|| (String::new(), String::new(), false));

    let raw_output = if !stdout.is_empty() || !stderr.is_empty() {
        match (stdout.trim_end(), stderr.trim_end()) {
            ("", s) => s.to_string(),
            (s, "") => s.to_string(),
            (o, e) => format!("{o}\n{e}"),
        }
    } else {
        content_text.to_string()
    };

    let exit_code = if is_error || interrupted { 1 } else { 0 };
    let formatted_output = if raw_output.trim().is_empty() {
        format!("$ {command}\nexit code: {exit_code}")
    } else {
        format!("$ {command}\n{raw_output}\n\nexit code: {exit_code}")
    };

    let mut item = Map::new();
    item.insert("id".into(), Value::String(format!("{tool_use_id}:result")));
    item.insert("type".into(), Value::String("toolResult".into()));
    item.insert("tool".into(), Value::String(tool_call.name.clone()));
    item.insert("name".into(), Value::String(tool_call.name.clone()));
    item.insert("output".into(), Value::String(formatted_output));
    item.insert("command".into(), Value::String(command.to_string()));
    if is_probably_diff_text(&raw_output) {
        item.insert("diff".into(), Value::String(raw_output));
    }
    Value::Object(item)
}

fn synthetic_file_tool_result(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    is_error: bool,
    content_text: &str,
    tool_use_result: Option<&Value>,
) -> Value {
    let mut item = Map::new();
    item.insert("id".into(), Value::String(format!("{tool_use_id}:result")));
    item.insert("type".into(), Value::String("toolResult".into()));
    item.insert("tool".into(), Value::String(tool_call.name.clone()));
    item.insert("name".into(), Value::String(tool_call.name.clone()));
    item.insert("input".into(), tool_call.input.clone());

    if let Some(result) = tool_use_result.cloned() {
        item.insert("result".into(), result);
    }
    if !content_text.trim().is_empty() {
        item.insert("output".into(), Value::String(content_text.to_string()));
        if is_probably_diff_text(content_text) {
            item.insert("diff".into(), Value::String(content_text.to_string()));
        }
    } else if is_error {
        item.insert("output".into(), Value::String("tool failed".into()));
    }
    Value::Object(item)
}

pub(super) fn synthetic_tool_result_line(
    tool_call: &ClaudeToolCall,
    tool_use_id: &str,
    part: &Value,
    tool_use_result: Option<&Value>,
) -> Option<String> {
    let item = synthetic_tool_result_item(tool_call, tool_use_id, part, tool_use_result)?;
    Some(
        json!({
            "method": "item/completed",
            "params": {
                "item": item
            }
        })
        .to_string(),
    )
}

pub(super) fn fallback_tool_result_item(tool_use_id: &str, part: &Value) -> Option<Value> {
    let content_text = value_to_string(part.get("content")?);
    if content_text.trim().is_empty() {
        return None;
    }
    Some(json!({
        "id": format!("{tool_use_id}:result"),
        "type": "toolResult",
        "output": content_text,
    }))
}

// --- Shared helpers ---

pub(super) fn value_to_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(value_to_string)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

pub(super) fn is_probably_diff_text(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("diff --git ")
        || trimmed.starts_with("@@ ")
        || (trimmed.contains("\n@@ ") && (trimmed.contains("\n+++ ") || trimmed.contains("\n--- ")))
        || (trimmed.contains('\n') && trimmed.contains("\n+++ ") && trimmed.contains("\n--- "))
}
