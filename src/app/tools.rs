use std::collections::HashSet;

use serde_json::{json, Value};

use super::Role;
pub(super) use super::tool_diff::{command_execution_diff_output, extract_diff_blocks};
use super::tool_diff::is_probably_diff_text;
use super::tool_shell::{
    command_execution_action_command, compact_command_path, normalize_shell_command,
};
pub(super) use super::tool_shell::{
    command_summary_from_shell_cmd, parse_ssh_remote_command, strip_terminal_controls,
    strip_terminal_controls_preserving_sgr,
};

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

    if let Some(s) = first_string_at_paths(item, &[&["aggregatedOutput"]]) {
        let cleaned = strip_terminal_controls_preserving_sgr(&s);
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }

    if let Some(s) = first_string_at_paths(
        item,
        &[
            &["output"],
            &["text"],
            &["result"],
            &["metadata", "output"],
            &["metadata", "result"],
            &["state", "output"],
            &["formattedOutput"],
        ],
    ) {
        let cleaned = strip_terminal_controls_preserving_sgr(&s);
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }
    if let Some(s) = first_string_at_paths(
        item,
        &[&["stdout"], &["metadata", "stdout"], &["state", "stdout"]],
    ) {
        let cleaned = strip_terminal_controls_preserving_sgr(&s);
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }
    if let Some(s) = first_string_at_paths(
        item,
        &[&["stderr"], &["metadata", "stderr"], &["state", "stderr"]],
    ) {
        let cleaned = strip_terminal_controls_preserving_sgr(&s);
        if !cleaned.is_empty() {
            parts.push(cleaned);
        }
    }
    if let Some(code) = first_i64_at_paths(
        item,
        &[
            &["exitCode"],
            &["exit_code"],
            &["metadata", "exitCode"],
            &["metadata", "exit_code"],
            &["state", "exitCode"],
            &["durationMs"],
        ],
    ) {
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
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

pub(super) fn command_summary_from_parsed_cmd(msg: &Value) -> Option<(String, String)> {
    let call_id = msg.get("call_id").and_then(Value::as_str)?.to_string();
    let parsed = msg.get("parsed_cmd").and_then(Value::as_array)?;
    let cwd = msg.get("cwd").and_then(Value::as_str);

    for part in parsed {
        let Some(obj) = part.as_object() else {
            continue;
        };
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

        let path_display = if path.is_empty() {
            None
        } else {
            Some(compact_command_path(path, cwd))
        };

        let mut out = if kind == "read" {
            "→ Read".to_string()
        } else if matches!(
            kind.as_str(),
            "grep" | "rg" | "search" | "glob" | "find" | "codesearch"
        ) {
            "✱ Search".to_string()
        } else if matches!(kind.as_str(), "list" | "listfiles" | "list_files" | "ls") {
            "→ List".to_string()
        } else if matches!(
            kind.as_str(),
            "write" | "edit" | "applypatch" | "apply_patch" | "replace"
        ) {
            "← Edit".to_string()
        } else if matches!(kind.as_str(), "diff" | "gitdiff" | "git_diff") {
            "Δ Diff".to_string()
        } else if let Some(cmd) = cmd {
            let Some(summary) = command_summary_from_shell_cmd(cmd, cwd) else {
                continue;
            };
            summary
        } else {
            continue;
        };

        if out == "→ Read"
            || out == "✱ Search"
            || out == "→ List"
            || out == "← Edit"
            || out == "Δ Diff"
        {
            if let Some(display) = path_display {
                out.push(' ');
                out.push_str(&display);
            }
        }

        if let Some(args) = format_input_brackets(
            obj,
            &[
                "type", "cmd", "command", "path", "filePath", "file", "name", "cwd",
            ],
        ) {
            out.push(' ');
            out.push_str(&args);
        }

        return Some((call_id, out));
    }

    None
}

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
