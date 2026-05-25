// --- Command Summaries ---
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

    let mut out = if paths.is_empty() {
        "✱ Search".to_string()
    } else {
        format!("✱ Search {}", paths.join(" "))
    };
    if let Some(p) = pattern.filter(|p| !p.is_empty()) {
        out.push_str(&format!(" [pattern={p}]"));
    }
    Some(out)
}

fn parse_shell_semeja_summary(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let tokens = shlex_split(cmd)?;
    let first = tokens
        .first()?
        .rsplit('/')
        .next()
        .unwrap_or(tokens.first()?);
    if first != "semeja" {
        return None;
    }

    let subcommand = tokens.get(1).map(String::as_str)?;
    match subcommand {
        "search" => parse_shell_semeja_search_summary(&tokens, cwd),
        "find-related" => parse_shell_semeja_related_summary(&tokens, cwd),
        _ => Some("✱ Search".to_string()),
    }
}

fn parse_shell_semeja_search_summary(tokens: &[String], cwd: Option<&str>) -> Option<String> {
    let args: Vec<&String> = tokens
        .iter()
        .skip(2)
        .filter(|token| !token.starts_with('-'))
        .collect();
    let query = args.first().map(|token| strip_shell_quotes(token));
    let paths: Vec<String> = args
        .iter()
        .skip(1)
        .map(|token| compact_command_path(strip_shell_quotes(token), cwd))
        .collect();

    let mut out = if paths.is_empty() {
        "✱ Search".to_string()
    } else {
        format!("✱ Search {}", paths.join(" "))
    };
    if let Some(query) = query.filter(|query| !query.is_empty()) {
        out.push_str(&format!(" [pattern={query}]"));
    }
    Some(out)
}

fn parse_shell_semeja_related_summary(tokens: &[String], cwd: Option<&str>) -> Option<String> {
    let path = tokens.get(2).map(|token| compact_command_path(token, cwd));
    let line = tokens.get(3).map(|token| strip_shell_quotes(token));
    let mut out = path
        .filter(|path| !path.is_empty())
        .map(|path| format!("✱ Search {path}"))
        .unwrap_or_else(|| "✱ Search".to_string());
    if let Some(line) = line.filter(|line| !line.is_empty()) {
        out.push_str(&format!(" [line={line}]"));
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

    let suffix = parse_sed_range(script)
        .map(|r| format!(" {r}"))
        .unwrap_or_default();
    Some(format!("✱ Search {path}{suffix}"))
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
    if lower.starts_with("rg ") {
        return parse_shell_search_summary(cmd, "rg", cwd).or(Some("✱ Search".to_string()));
    }
    if lower.starts_with("grep ") {
        return parse_shell_search_summary(cmd, "grep", cwd).or(Some("✱ Search".to_string()));
    }
    if lower.starts_with("semeja ") {
        return parse_shell_semeja_summary(cmd, cwd).or(Some("✱ Search".to_string()));
    }
    if lower.starts_with("fd ") || lower.starts_with("find ") || lower.starts_with("git grep ") {
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
        let sub = lhs.split_once(' ').map(|(_, r)| r).unwrap_or("").trim();
        let label = first_non_option_token(sub)
            .map(strip_shell_quotes)
            .filter(|p| !p.is_empty())
            .map(|p| format!("→ Read {}", compact_command_path(p, cwd)))
            .unwrap_or_else(|| "→ Read".to_string());
        return Some(label);
    }

    if lhs_lower.starts_with("sed ") {
        return parse_shell_sed_summary(lhs, cwd).or(Some("✱ Search".to_string()));
    }

    None
}
