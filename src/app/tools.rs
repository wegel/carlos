use std::collections::HashSet;

use serde_json::{json, Value};
use shlex::split as shlex_split;

use super::{DiffBlock, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedSshCommand {
    pub(super) destination: String,
    pub(super) remote_command: String,
}

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

pub(super) fn is_probably_diff_text(text: &str) -> bool {
    let t = text.trim();
    t.starts_with("diff --git ")
        || t.starts_with("@@ ")
        || (t.contains("\n@@ ") && (t.contains("\n+++ ") || t.contains("\n--- ")))
        || (t.contains('\n') && t.contains("\n+++ ") && t.contains("\n--- "))
}

pub(super) fn infer_file_path_from_object(obj: &serde_json::Map<String, Value>) -> Option<String> {
    for key in ["filePath", "path", "file", "filename"] {
        if let Some(v) = obj.get(key).and_then(Value::as_str) {
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub(super) fn collect_diff_blocks_recursive(
    value: &Value,
    current_file_path: Option<&str>,
    seen: &mut HashSet<String>,
    out: &mut Vec<DiffBlock>,
) {
    match value {
        Value::Object(obj) => {
            let inferred_path = infer_file_path_from_object(obj);
            let local_path = inferred_path.as_deref().or(current_file_path);

            if let Some(diff) = obj.get("diff").and_then(Value::as_str) {
                if !diff.is_empty() && is_probably_diff_text(diff) {
                    let key = format!("{}::{}", local_path.unwrap_or(""), diff);
                    if seen.insert(key) {
                        out.push(DiffBlock {
                            file_path: local_path.map(ToOwned::to_owned),
                            diff: diff.to_string(),
                        });
                    }
                }
            }

            for nested in obj.values() {
                collect_diff_blocks_recursive(nested, local_path, seen, out);
            }
        }
        Value::Array(arr) => {
            for nested in arr {
                collect_diff_blocks_recursive(nested, current_file_path, seen, out);
            }
        }
        _ => {}
    }
}

pub(super) fn extract_diff_blocks(item: &Value) -> Vec<DiffBlock> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    collect_diff_blocks_recursive(item, None, &mut seen, &mut out);
    out
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

pub(super) fn command_execution_action_command(item: &Value) -> Option<String> {
    let actions = item.get("commandActions").and_then(Value::as_array)?;
    for a in actions {
        if let Some(cmd) = a.get("command").and_then(Value::as_str) {
            if !cmd.is_empty() {
                return Some(cmd.to_string());
            }
        }
    }
    None
}

pub(super) fn normalize_shell_command(raw: &str) -> String {
    if let Some(pos) = raw.find(" -lc '") {
        let s = &raw[(pos + 6)..];
        if let Some(end) = s.rfind('\'') {
            return s[..end].to_string();
        }
    }
    raw.to_string()
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

pub(super) fn parse_ssh_remote_command(command: &str) -> Option<ParsedSshCommand> {
    let normalized = normalize_shell_command(command);
    let tokens = shlex_split(&normalized)?;
    let first = tokens.first()?;
    let executable = first.rsplit('/').next().unwrap_or(first);
    if executable != "ssh" {
        return None;
    }

    let mut idx = 1usize;
    let mut destination = None;
    while idx < tokens.len() {
        let token = &tokens[idx];
        if token == "--" {
            idx += 1;
            continue;
        }
        if token.starts_with('-') {
            idx += if ssh_option_takes_value(token) { 2 } else { 1 };
            continue;
        }
        destination = Some(token.clone());
        idx += 1;
        break;
    }

    let destination = destination?;
    if idx >= tokens.len() {
        return None;
    }

    let remote_command = tokens[idx..].join(" ").trim().to_string();
    if remote_command.is_empty() {
        return None;
    }

    Some(ParsedSshCommand {
        destination,
        remote_command,
    })
}

fn ssh_option_takes_value(token: &str) -> bool {
    matches!(
        token,
        "-b" | "-c"
            | "-D"
            | "-E"
            | "-e"
            | "-F"
            | "-I"
            | "-i"
            | "-J"
            | "-L"
            | "-l"
            | "-m"
            | "-O"
            | "-o"
            | "-p"
            | "-Q"
            | "-R"
            | "-S"
            | "-W"
            | "-w"
    )
}

pub(super) fn strip_terminal_controls_preserving_sgr(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == 0xC2 && i + 1 < bytes.len() && bytes[i + 1] == 0x9b {
            i += 2;
            let params_start = i;
            while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'm' {
                out.push_str("\u{1b}[");
                out.push_str(std::str::from_utf8(&bytes[params_start..i]).unwrap_or_default());
                out.push('m');
            }
            i = i.saturating_add(1);
            continue;
        }

        match bytes[i] {
            0x1b => {
                if i + 1 >= bytes.len() {
                    break;
                }
                match bytes[i + 1] {
                    b'[' => {
                        let seq_start = i;
                        i += 2;
                        while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                            i += 1;
                        }
                        if i < bytes.len() && bytes[i] == b'm' {
                            out.push_str(&text[seq_start..=i]);
                        }
                        i = i.saturating_add(1);
                    }
                    b']' => {
                        i += 2;
                        while i < bytes.len() {
                            if bytes[i] == 0x07 {
                                i += 1;
                                break;
                            }
                            if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                    }
                    b'P' | b'X' | b'^' | b'_' => {
                        i += 2;
                        while i < bytes.len() {
                            if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                                i += 2;
                                break;
                            }
                            i += 1;
                        }
                    }
                    _ => {
                        if bytes[i + 1].is_ascii() {
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                }
            }
            b'\n' | b'\r' | b'\t' => {
                out.push(bytes[i] as char);
                i += 1;
            }
            b if b < 0x20 || b == 0x7f => {
                i += 1;
            }
            _ => {
                let Some(rest) = text.get(i..) else {
                    i += 1;
                    continue;
                };
                let ch = rest.chars().next().expect("valid utf-8");
                out.push(ch);
                i += ch.len_utf8();
            }
        }
    }

    out
}

pub(super) fn strip_terminal_controls(text: &str) -> String {
    let preserved = strip_terminal_controls_preserving_sgr(text);
    let bytes = preserved.as_bytes();
    let mut out = String::with_capacity(preserved.len());
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            i += 2;
            while i < bytes.len() && !(0x40..=0x7e).contains(&bytes[i]) {
                i += 1;
            }
            i = i.saturating_add(1);
            continue;
        }

        let Some(rest) = preserved.get(i..) else {
            i += 1;
            continue;
        };
        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }

    out
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

pub(super) fn command_execution_diff_output(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) != Some("commandExecution") {
        return None;
    }
    let candidates = [
        item.get("aggregatedOutput").and_then(Value::as_str),
        item.get("formattedOutput").and_then(Value::as_str),
        item.get("stdout").and_then(Value::as_str),
        item.get("output").and_then(Value::as_str),
    ];
    for c in candidates.into_iter().flatten() {
        if !c.trim().is_empty() && is_probably_diff_text(c) {
            return Some(c.to_string());
        }
    }
    None
}

pub(super) fn compact_command_path(path: &str, cwd: Option<&str>) -> String {
    if let Some(cwd) = cwd {
        if let Some(rest) = path.strip_prefix(cwd) {
            if let Some(trimmed) = rest.strip_prefix('/') {
                if !trimmed.is_empty() {
                    return trimmed.to_string();
                }
            }
        }
    }
    path.to_string()
}

pub(super) fn first_non_option_token(command: &str) -> Option<&str> {
    command
        .split_whitespace()
        .find(|t| !t.is_empty() && !t.starts_with('-'))
}

pub(super) fn strip_shell_quotes(token: &str) -> &str {
    let t = token.trim();
    if t.len() >= 2
        && ((t.starts_with('\'') && t.ends_with('\'')) || (t.starts_with('"') && t.ends_with('"')))
    {
        return &t[1..(t.len() - 1)];
    }
    t
}

fn parse_shell_search_summary(cmd: &str, program: &str, cwd: Option<&str>) -> Option<String> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.first().copied()? != program {
        return None;
    }

    let mut i = 1usize;
    while i < tokens.len() && tokens[i].starts_with('-') {
        i += 1;
    }

    let pattern = tokens.get(i).map(|t| strip_shell_quotes(t));
    let path = tokens
        .iter()
        .skip(i.saturating_add(1))
        .find(|t| !t.starts_with('-'))
        .map(|t| compact_command_path(strip_shell_quotes(t), cwd));

    let mut out = "✱ Search".to_string();
    if let Some(path) = path {
        out.push(' ');
        out.push_str(&path);
    }
    if let Some(pattern) = pattern {
        if !pattern.is_empty() {
            out.push_str(&format!(" [pattern={pattern}]"));
        }
    }
    Some(out)
}

fn parse_shell_sed_summary(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.first().copied()? != "sed" {
        return None;
    }

    let mut i = 1usize;
    while i < tokens.len() && tokens[i].starts_with('-') {
        i += 1;
    }
    let script = strip_shell_quotes(tokens.get(i)?);
    let path = compact_command_path(strip_shell_quotes(tokens.get(i + 1)?), cwd);

    let mut out = format!("✱ Search {path}");
    if let Some(range) = script.strip_suffix('p') {
        let range = range.trim();
        if let Some((start, end)) = range.split_once(',') {
            if !start.is_empty() && !end.is_empty() {
                out.push_str(&format!(" [lines={start}..{end}]"));
                return Some(out);
            }
        }
        if !range.is_empty() {
            out.push_str(&format!(" [lines={range}]"));
            return Some(out);
        }
    }
    Some(out)
}

pub(super) fn command_summary_from_shell_cmd(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let cmd = normalize_shell_command(cmd);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }

    let lower = cmd.to_ascii_lowercase();
    if lower.starts_with("git diff ") || lower == "git diff" || lower.starts_with("diff ") {
        return Some("Δ Diff".to_string());
    }
    if lower.starts_with("apply_patch") || lower.starts_with("patch ") {
        return Some("← Edit".to_string());
    }
    if lower.starts_with("rg ")
        || lower.starts_with("grep ")
        || lower.starts_with("fd ")
        || lower.starts_with("find ")
        || lower.starts_with("git grep ")
    {
        if lower.starts_with("rg ") {
            return parse_shell_search_summary(cmd, "rg", cwd).or(Some("✱ Search".to_string()));
        }
        if lower.starts_with("grep ") {
            return parse_shell_search_summary(cmd, "grep", cwd).or(Some("✱ Search".to_string()));
        }
        return Some("✱ Search".to_string());
    }

    let lhs = cmd.split('|').next().unwrap_or(cmd).trim();
    let lhs_lower = lhs.to_ascii_lowercase();

    if lhs_lower.starts_with("nl ")
        || lhs_lower.starts_with("cat ")
        || lhs_lower.starts_with("bat ")
        || lhs_lower.starts_with("head ")
        || lhs_lower.starts_with("tail ")
    {
        let sub = lhs
            .split_once(' ')
            .map(|(_, rest)| rest)
            .unwrap_or("")
            .trim();
        if let Some(path_token) = first_non_option_token(sub) {
            let path = strip_shell_quotes(path_token);
            if !path.is_empty() {
                return Some(format!("→ Read {}", compact_command_path(path, cwd)));
            }
        }
        return Some("→ Read".to_string());
    }

    if lhs_lower.starts_with("sed ") {
        return parse_shell_sed_summary(lhs, cwd).or(Some("✱ Search".to_string()));
    }

    None
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
