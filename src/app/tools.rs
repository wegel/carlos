//! Tool-call parsing, JSON extraction, and type classification for transcript items.
//! Display formatting lives in `tool_format`.

use serde_json::{json, Value};

use super::tool_diff::is_probably_diff_text;
pub(super) use super::tool_diff::{command_execution_diff_output, extract_diff_blocks};
use super::tool_shell::{
    command_execution_action_command, normalize_shell_command,
};
pub(super) use super::tool_shell::{
    command_summary_from_shell_cmd, strip_terminal_controls,
    strip_terminal_controls_preserving_sgr,
};
pub(super) use super::tool_format::{
    command_summary_from_parsed_cmd, format_tool_item,
};
use super::Role;

// --- JSON Utilities ---

pub(super) fn is_tool_call_type(kind: &str) -> bool {
    matches!(
        kind,
        "toolCall" | "tool_call" | "toolInvocation" | "functionCall" | "mcpToolCall"
    )
}

pub(super) fn is_tool_output_type(kind: &str) -> bool {
    matches!(
        kind,
        "toolResult" | "toolOutput" | "tool_result" | "functionCallOutput" | "mcpToolResult"
    )
}

pub(super) fn role_for_tool_type(kind: &str) -> Option<Role> {
    if kind == "commandExecution" {
        return Some(Role::ToolCall);
    }
    if is_tool_call_type(kind) {
        return Some(Role::ToolCall);
    }
    if is_tool_output_type(kind) {
        return Some(Role::ToolOutput);
    }
    None
}

pub(super) fn value_at_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut cur = value;
    for key in path {
        cur = cur.get(*key)?;
    }
    Some(cur)
}

pub(super) fn first_string_at_paths(value: &Value, paths: &[&[&str]]) -> Option<String> {
    for path in paths {
        if let Some(s) = value_at_path(value, path).and_then(Value::as_str) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

pub(super) fn first_i64_at_paths(value: &Value, paths: &[&[&str]]) -> Option<i64> {
    for path in paths {
        if let Some(v) = value_at_path(value, path) {
            if let Some(n) = v.as_i64() {
                return Some(n);
            }
            if let Some(n) = v.as_u64() {
                return i64::try_from(n).ok();
            }
        }
    }
    None
}

// --- Tool Extraction ---

pub(super) fn tool_name(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["tool"],
            &["name"],
            &["input", "tool"],
            &["input", "name"],
            &["function", "name"],
        ],
    )
}

pub(super) fn tool_command(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) == Some("commandExecution") {
        if let Some(cmd) = command_execution_action_command(item) {
            return Some(cmd);
        }
        if let Some(raw) = item.get("command").and_then(Value::as_str) {
            if !raw.is_empty() {
                return Some(normalize_shell_command(raw));
            }
        }
    }

    first_string_at_paths(
        item,
        &[
            &["command"],
            &["input", "command"],
            &["action", "command"],
            &["args", "command"],
            &["arguments", "command"],
            &["metadata", "command"],
        ],
    )
}

pub(super) fn tool_reasoning(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["reasoning"],
            &["input", "reasoning"],
            &["metadata", "reasoning"],
            &["thought"],
        ],
    )
}

pub(super) fn tool_description(item: &Value) -> Option<String> {
    first_string_at_paths(
        item,
        &[
            &["description"],
            &["input", "description"],
            &["metadata", "description"],
            &["title"],
            &["metadata", "title"],
        ],
    )
}

pub(super) fn tool_output_text(item: &Value) -> Option<String> {
    let mut parts = Vec::new();

    push_cleaned(item, &mut parts, &[&["aggregatedOutput"]]);
    push_cleaned(item, &mut parts, &[
        &["output"], &["text"], &["result"],
        &["metadata", "output"], &["metadata", "result"],
        &["state", "output"], &["formattedOutput"],
    ]);
    push_cleaned(item, &mut parts, &[
        &["stdout"], &["metadata", "stdout"], &["state", "stdout"],
    ]);
    push_cleaned(item, &mut parts, &[
        &["stderr"], &["metadata", "stderr"], &["state", "stderr"],
    ]);

    if let Some(code) = first_i64_at_paths(item, &[
        &["exitCode"], &["exit_code"],
        &["metadata", "exitCode"], &["metadata", "exit_code"],
        &["state", "exitCode"], &["durationMs"],
    ]) {
        if item.get("durationMs").is_some() && item.get("exitCode").is_none() {
            parts.push(format!("duration: {code} ms"));
        } else {
            parts.push(format!("exit code: {code}"));
        }
    }
    if let Some(code) = first_i64_at_paths(item, &[&["exitCode"], &["exit_code"]]) {
        if !parts.iter().any(|p| p.starts_with("exit code:")) {
            parts.push(format!("exit code: {code}"));
        }
    }

    parts.retain(|s| !s.trim().is_empty());
    if parts.is_empty() { None } else { Some(parts.join("\n")) }
}

fn push_cleaned(item: &Value, parts: &mut Vec<String>, paths: &[&[&str]]) {
    if let Some(s) = first_string_at_paths(item, paths) {
        let cleaned = strip_terminal_controls_preserving_sgr(&s);
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }
}

pub(super) fn collect_text_parts(content: &[Value]) -> Vec<&str> {
    let mut text_parts = Vec::new();
    for part in content {
        if part.get("type").and_then(Value::as_str) != Some("text") {
            continue;
        }
        if let Some(t) = part.get("text").and_then(Value::as_str) {
            text_parts.push(t);
        }
    }
    text_parts
}

pub(super) fn item_text_from_content(item: &Value) -> Option<String> {
    let content = item.get("content").and_then(Value::as_array)?;
    let text_parts = collect_text_parts(content);
    if text_parts.is_empty() {
        None
    } else {
        Some(text_parts.join("\n"))
    }
}

pub(super) fn parse_arguments_value(arguments: &Value) -> Option<Value> {
    match arguments {
        Value::Null => None,
        Value::String(s) => {
            if s.trim().is_empty() {
                None
            } else {
                serde_json::from_str::<Value>(s)
                    .ok()
                    .or_else(|| Some(Value::String(s.clone())))
            }
        }
        other => Some(other.clone()),
    }
}

pub(super) fn raw_function_call_to_tool_item(item: &Value) -> Option<(String, Value)> {
    if item.get("type").and_then(Value::as_str) != Some("function_call") {
        return None;
    }
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))?
        .to_string();
    let name = item.get("name").and_then(Value::as_str).unwrap_or("");

    let mut out = serde_json::Map::new();
    out.insert(
        "type".to_string(),
        Value::String(if name == "exec_command" {
            "commandExecution".to_string()
        } else {
            "toolCall".to_string()
        }),
    );
    if !name.is_empty() {
        out.insert("tool".to_string(), Value::String(name.to_string()));
        out.insert("name".to_string(), Value::String(name.to_string()));
    }

    if let Some(args) = item.get("arguments").and_then(parse_arguments_value) {
        if let Value::Object(obj) = &args {
            out.insert("input".to_string(), Value::Object(obj.clone()));
            if name == "exec_command" {
                if let Some(cmd) = obj.get("cmd").and_then(Value::as_str) {
                    out.insert("command".to_string(), Value::String(cmd.to_string()));
                    out.insert(
                        "commandActions".to_string(),
                        json!([{ "type": "unknown", "command": cmd }]),
                    );
                }
            }
        } else {
            out.insert("input".to_string(), json!({ "value": args }));
        }
    }

    Some((call_id, Value::Object(out)))
}

pub(super) fn raw_function_call_output_to_tool_item(item: &Value) -> Option<(String, Value)> {
    if item.get("type").and_then(Value::as_str) != Some("function_call_output") {
        return None;
    }
    let call_id = item
        .get("call_id")
        .and_then(Value::as_str)
        .or_else(|| item.get("id").and_then(Value::as_str))?
        .to_string();

    let output_value = item.get("output").cloned().unwrap_or(Value::Null);
    let mut out = serde_json::Map::new();
    out.insert("type".to_string(), Value::String("toolOutput".to_string()));
    match output_value {
        Value::String(s) => {
            out.insert("output".to_string(), Value::String(s.clone()));
            if is_probably_diff_text(&s) {
                out.insert("diff".to_string(), Value::String(s));
            }
        }
        Value::Object(obj) => {
            for (k, v) in obj {
                out.insert(k, v);
            }
        }
        Value::Null => {}
        other => {
            out.insert("output".to_string(), other);
        }
    }

    Some((call_id, Value::Object(out)))
}
