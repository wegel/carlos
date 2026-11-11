//! Formats native tool results for the shared transcript renderer.

use serde_json::{json, Map, Value};

use super::types::ClaudeToolCall;

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
            tool_call,
            tool_use_id,
            is_error,
            &content_text,
            tool_use_result,
        ));
    }
    if !is_error && !matches!(lower.as_str(), "read" | "write" | "edit") {
        return None;
    }
    Some(synthetic_file_tool_result(
        tool_call,
        tool_use_id,
        is_error,
        &content_text,
        tool_use_result,
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
    item.insert("isError".into(), json!(is_error || interrupted));
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
    item.insert("isError".into(), json!(is_error));

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
        "isError": part.get("is_error").and_then(Value::as_bool).unwrap_or(false),
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
