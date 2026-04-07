//! Display formatting helpers for tool-call and tool-output transcript items.

use std::collections::HashSet;

use serde_json::Value;

use super::tool_shell::{command_summary_from_shell_cmd, compact_command_path, parse_ssh_remote_command};
use super::tools::{
    tool_command, tool_description, tool_name, tool_output_text, tool_reasoning, value_at_path,
};
use super::Role;

// --- JSON Summary Helpers ---

pub(super) fn compact_json_summary(value: &Value, max_chars: usize) -> Option<String> {
    let mut s = serde_json::to_string(value).ok()?;
    if s.len() > max_chars {
        s.truncate(max_chars.saturating_sub(1));
        s.push('…');
    }
    Some(s)
}

pub(super) fn tool_input_object(item: &Value) -> Option<&serde_json::Map<String, Value>> {
    value_at_path(item, &["input"]).and_then(Value::as_object)
}

pub(super) fn inline_value(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(s.clone())
            }
        }
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => compact_json_summary(value, 64),
    }
}

pub(super) fn format_input_brackets(
    input: &serde_json::Map<String, Value>,
    skip_keys: &[&str],
) -> Option<String> {
    let skip: HashSet<&str> = skip_keys.iter().copied().collect();
    let mut keys: Vec<&str> = input
        .keys()
        .map(String::as_str)
        .filter(|k| !skip.contains(*k))
        .collect();
    keys.sort_unstable();

    let mut parts = Vec::new();
    for key in keys {
        let Some(val) = input.get(key).and_then(inline_value) else {
            continue;
        };
        if val.is_empty() {
            continue;
        }
        parts.push(format!("{key}={val}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("[{}]", parts.join(" ")))
    }
}

// --- Name / Icon Formatting ---

pub(super) fn titlecase_tool_name(name: &str) -> String {
    let normalized = name.replace(['_', '-'], " ");
    let mut out = String::new();
    for (i, word) in normalized.split_whitespace().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut chars = word.chars();
        if let Some(first) = chars.next() {
            out.push(first.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        name.to_string()
    } else {
        out
    }
}

pub(super) fn tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "read" | "list" => "→",
        "write" | "edit" | "applypatch" | "apply_patch" => "←",
        "grep" | "glob" | "codesearch" => "✱",
        "task" => "#",
        _ => "◇",
    }
}

// --- Inline Tool Call Formatting ---

pub(super) fn format_tool_call_inline(item: &Value, tool_name: &str) -> Option<String> {
    let lower = tool_name.to_ascii_lowercase();
    let input = tool_input_object(item);

    if lower.as_str() == "read" {
        let path = input
            .and_then(|obj| {
                obj.get("filePath")
                    .and_then(Value::as_str)
                    .or_else(|| obj.get("path").and_then(Value::as_str))
            })
            .unwrap_or("");
        let mut out = format!("{} Read {}", tool_icon(&lower), path);
        if let Some(args) = input.and_then(|obj| {
            format_input_brackets(
                obj,
                &[
                    "filePath",
                    "path",
                    "tool",
                    "name",
                    "command",
                    "description",
                    "reasoning",
                ],
            )
        }) {
            if !args.is_empty() {
                out.push(' ');
                out.push_str(&args);
            }
        }
        return Some(out.trim_end().to_string());
    }

    if let Some(obj) = input {
        let mut out = format!("{} {}", tool_icon(&lower), titlecase_tool_name(tool_name));
        if let Some(path) = obj
            .get("filePath")
            .and_then(Value::as_str)
            .or_else(|| obj.get("path").and_then(Value::as_str))
        {
            if !path.is_empty() {
                out.push(' ');
                out.push_str(path);
            }
        }

        if let Some(args) = format_input_brackets(
            obj,
            &[
                "filePath",
                "path",
                "tool",
                "name",
                "command",
                "description",
                "reasoning",
                "content",
                "patch",
                "old_string",
                "new_string",
                "text",
            ],
        ) {
            out.push(' ');
            out.push_str(&args);
        }
        return Some(out);
    }

    Some(format!(
        "{} {}",
        tool_icon(&lower),
        titlecase_tool_name(tool_name)
    ))
}

// --- Input Summary ---

pub(super) fn tool_input_summary(item: &Value) -> Option<String> {
    if let Some(obj) = value_at_path(item, &["input"]).and_then(Value::as_object) {
        let mut fields = Vec::new();
        for key in [
            "filePath",
            "path",
            "pattern",
            "query",
            "url",
            "method",
            "subagent_type",
        ] {
            if let Some(v) = obj.get(key).and_then(Value::as_str) {
                if !v.is_empty() {
                    fields.push(format!("{key}={v}"));
                }
            }
        }
        if !fields.is_empty() {
            return Some(fields.join(" "));
        }
        return compact_json_summary(&Value::Object(obj.clone()), 220);
    }
    None
}

// --- Top-Level Item Formatter ---

pub(super) fn format_tool_item(item: &Value, role: Role) -> Option<String> {
    match role {
        Role::ToolCall => {
            if let Some(cmd) = tool_command(item) {
                let mut lines = Vec::new();
                if let Some(ssh) = parse_ssh_remote_command(&cmd) {
                    lines.push(format!("remote exec on {}", ssh.destination));
                    if let Some(reason) = tool_reasoning(item) {
                        lines.push(format!("Thinking: {reason}"));
                    }
                    if let Some(desc) = tool_description(item) {
                        lines.push(format!("# {desc}"));
                    }
                    lines.push(format!("$ {}", ssh.remote_command));
                    return Some(lines.join("\n"));
                }

                lines.push(format!("run `{cmd}`"));
                if let Some(reason) = tool_reasoning(item) {
                    lines.push(format!("Thinking: {reason}"));
                }
                if let Some(desc) = tool_description(item) {
                    lines.push(format!("# {desc}"));
                }
                lines.push(format!("$ {cmd}"));
                return Some(lines.join("\n"));
            }

            if let Some(name) = tool_name(item) {
                if let Some(inline) = format_tool_call_inline(item, &name) {
                    return Some(inline);
                }
                if let Some(input) = tool_input_summary(item) {
                    return Some(format!("{} {}", titlecase_tool_name(&name), input));
                }
                return Some(titlecase_tool_name(&name));
            }

            None
        }
        Role::ToolOutput => {
            if item.get("type").and_then(Value::as_str) == Some("commandExecution") {
                let command = tool_command(item);
                let output = tool_output_text(item);

                return match (command, output) {
                    (Some(cmd), Some(out)) => {
                        if let Some(ssh) = parse_ssh_remote_command(&cmd) {
                            Some(format!(
                                "remote exec on {}\n$ {}\n{}",
                                ssh.destination, ssh.remote_command, out
                            ))
                        } else {
                            Some(format!("$ {cmd}\n{out}"))
                        }
                    }
                    (Some(cmd), None) => {
                        if let Some(ssh) = parse_ssh_remote_command(&cmd) {
                            Some(format!(
                                "remote exec on {}\n$ {}",
                                ssh.destination, ssh.remote_command
                            ))
                        } else {
                            Some(format!("$ {cmd}"))
                        }
                    }
                    (None, Some(out)) => Some(out),
                    (None, None) => None,
                };
            }

            if let Some(out) = tool_output_text(item) {
                return Some(out);
            }
            None
        }
        _ => None,
    }
}

// --- Command Summarization from Parsed Command ---

pub(super) fn command_summary_from_parsed_cmd(msg: &Value) -> Option<(String, String)> {
    let call_id = msg.get("call_id").and_then(Value::as_str)?.to_string();
    let parsed = msg.get("parsed_cmd").and_then(Value::as_array)?;
    let cwd = msg.get("cwd").and_then(Value::as_str);

    for part in parsed {
        let Some(obj) = part.as_object() else {
            continue;
        };
        if let Some(summary) = summarize_parsed_cmd_part(obj, cwd) {
            return Some((call_id, summary));
        }
    }
    None
}

fn summarize_parsed_cmd_part(
    obj: &serde_json::Map<String, Value>,
    cwd: Option<&str>,
) -> Option<String> {
    let kind = obj
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let cmd = obj
        .get("cmd")
        .and_then(Value::as_str)
        .or_else(|| obj.get("command").and_then(Value::as_str));

    let path = obj
        .get("path")
        .and_then(Value::as_str)
        .or_else(|| obj.get("filePath").and_then(Value::as_str))
        .or_else(|| obj.get("file").and_then(Value::as_str))
        .or_else(|| obj.get("name").and_then(Value::as_str))
        .unwrap_or("")
        .trim();

    let mut out = match kind.as_str() {
        "read" => "→ Read".to_string(),
        "grep" | "rg" | "search" | "glob" | "find" | "codesearch" => "✱ Search".to_string(),
        "list" | "listfiles" | "list_files" | "ls" => "→ List".to_string(),
        "write" | "edit" | "applypatch" | "apply_patch" | "replace" => "← Edit".to_string(),
        "diff" | "gitdiff" | "git_diff" => "Δ Diff".to_string(),
        _ => {
            let summary = command_summary_from_shell_cmd(cmd?, cwd)?;
            return Some(append_input_brackets(obj, summary));
        }
    };

    if !path.is_empty() {
        out.push(' ');
        out.push_str(&compact_command_path(path, cwd));
    }
    Some(append_input_brackets(obj, out))
}

fn append_input_brackets(obj: &serde_json::Map<String, Value>, mut out: String) -> String {
    if let Some(args) = format_input_brackets(
        obj,
        &["type", "cmd", "command", "path", "filePath", "file", "name", "cwd"],
    ) {
        out.push(' ');
        out.push_str(&args);
    }
    out
}
