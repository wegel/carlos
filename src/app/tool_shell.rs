use serde_json::Value;
use shlex::split as shlex_split;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParsedSshCommand {
    pub(super) destination: String,
    pub(super) remote_command: String,
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

fn tokens_without_program_options(tokens: &[String]) -> &[String] {
    let mut i = 1usize;
    while i < tokens.len() && tokens[i].starts_with('-') {
        i += 1;
    }
    &tokens[i..]
}

fn parse_shell_search_summary(cmd: &str, program: &str, cwd: Option<&str>) -> Option<String> {
    let tokens = shlex_split(cmd)?;
    if tokens.first().map(String::as_str)? != program {
        return None;
    }

    let args = tokens_without_program_options(&tokens);
    let pattern = args.first().map(|t| strip_shell_quotes(t));
    let paths: Vec<String> = args
        .iter()
        .skip(1)
        .filter(|t| !t.starts_with('-'))
        .map(|t| compact_command_path(strip_shell_quotes(t), cwd))
        .collect();

    let mut out = "✱ Search".to_string();
    if !paths.is_empty() {
        out.push(' ');
        out.push_str(&paths.join(" "));
    }
    if let Some(pattern) = pattern {
        if !pattern.is_empty() {
            out.push_str(&format!(" [pattern={pattern}]"));
        }
    }
    Some(out)
}

fn parse_sed_range(script: &str) -> Option<String> {
    if let Some(range) = script.strip_suffix('p') {
        let range = range.trim();
        if let Some((start, end)) = range.split_once(',') {
            if !start.is_empty() && !end.is_empty() {
                return Some(format!("[lines={start}..{end}]"));
            }
        }
        if !range.is_empty() {
            return Some(format!("[lines={range}]"));
        }
    }
    None
}

fn parse_shell_sed_summary(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let tokens = shlex_split(cmd)?;
    if tokens.first().map(String::as_str)? != "sed" {
        return None;
    }

    let args = tokens_without_program_options(&tokens);
    let script = strip_shell_quotes(args.first()?);
    let path = compact_command_path(strip_shell_quotes(args.get(1)?), cwd);

    let mut out = format!("✱ Search {path}");
    if let Some(range) = parse_sed_range(script) {
        out.push(' ');
        out.push_str(&range);
    }
    Some(out)
}

fn parse_shell_read_path(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let tokens = shlex_split(cmd)?;
    let first = tokens
        .first()?
        .rsplit('/')
        .next()
        .unwrap_or(tokens.first()?);
    if !matches!(first, "nl" | "cat" | "bat" | "head" | "tail") {
        return None;
    }

    tokens
        .iter()
        .skip(1)
        .rev()
        .find(|t| !t.starts_with('-'))
        .map(|t| compact_command_path(strip_shell_quotes(t), cwd))
}

fn parse_shell_pipeline_summary(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let (lhs, rhs) = cmd.split_once('|')?;
    let lhs = lhs.trim();
    let rhs = rhs.trim();

    if let Some(range) = shlex_split(rhs)
        .and_then(|tokens| tokens_without_program_options(&tokens).first().cloned())
        .and_then(|script| parse_sed_range(strip_shell_quotes(&script)))
    {
        if let Some(path) = parse_shell_read_path(lhs, cwd) {
            return Some(format!("✱ Search {path} {range}"));
        }
    }

    None
}

pub(super) fn command_summary_from_shell_cmd(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let cmd = normalize_shell_command(cmd);
    let cmd = cmd.trim();
    if cmd.is_empty() {
        return None;
    }

    if let Some(summary) = parse_shell_pipeline_summary(cmd, cwd) {
        return Some(summary);
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
