//! Per-program one-line summaries of shell commands shown in the tool feed.

use super::{
    compact_command_path, normalize_shell_command, shlex_split, strip_shell_quotes,
    tokens_without_program_options,
};

use crate::app::shell_command::{analyze_shell_command, ShellCommand};
use crate::app::shell_read::{is_print_range, read_paths, single_pipe};
use crate::app::shell_search::search_operands;
use crate::app::shell_words::{has_shell_operators, program_name};

fn parse_shell_search_summary(cmd: &str, program: &str, cwd: Option<&str>) -> Option<String> {
    let tokens = shlex_split(cmd)?;
    if tokens.first().map(String::as_str)? != program {
        return None;
    }

    let (pattern, paths) = search_operands(&tokens)?;
    let paths: Vec<String> = paths
        .iter()
        .map(|path| compact_command_path(path, cwd))
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

fn parse_shell_pipeline_summary(cmd: &str, cwd: Option<&str>) -> Option<String> {
    let (lhs, rhs) = single_pipe(cmd)?;
    let left = analyze_shell_command(lhs);
    let right = analyze_shell_command(rhs);
    let words = shlex_split(&right.text)?;
    if is_output_limiter(&words) {
        let search = shlex_split(&left.text)?;
        if matches!(program_name(search.first()?), "rg" | "grep") {
            return parse_shell_search_summary(&left.text, &search[0], cwd);
        }
    }
    let paths = read_paths(&left)?;
    if program_name(words.first()?) != "sed" || words.len() != 3 || words[1] != "-n" {
        return None;
    }
    let range = parse_sed_range(&words[2])?;
    Some(format!(
        "✱ Search {} {range}",
        paths
            .iter()
            .map(|path| compact_command_path(path, cwd))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn is_output_limiter(words: &[String]) -> bool {
    let Some(program) = words.first().map(|word| program_name(word)) else {
        return false;
    };
    let args: Vec<&str> = words.iter().skip(1).map(String::as_str).collect();
    if program == "sed" {
        return matches!(args.as_slice(), ["-n", script] if is_print_range(script));
    }
    if !matches!(program, "head" | "tail") {
        return false;
    }
    match args.as_slice() {
        [] => true,
        ["-n" | "--lines" | "-c" | "--bytes", count] => is_line_count(count),
        [flag] => ["--lines=", "--bytes=", "-n", "-c", "-"]
            .iter()
            .any(|prefix| flag.strip_prefix(prefix).is_some_and(is_line_count)),
        _ => false,
    }
}

fn is_line_count(count: &str) -> bool {
    let count = count
        .strip_prefix('+')
        .or_else(|| count.strip_prefix('-'))
        .unwrap_or(count);
    !count.is_empty() && count.bytes().all(|byte| byte.is_ascii_digit())
}

pub(in crate::app) fn command_summary_from_shell_cmd(
    cmd: &str,
    cwd: Option<&str>,
) -> Option<String> {
    let command = analyze_shell_command(cmd);
    let cmd = command.text.trim();
    if cmd.is_empty() {
        return None;
    }
    if has_shell_operators(cmd) {
        return parse_shell_pipeline_summary(cmd, cwd)
            .or_else(|| command.has_rtk.then(|| format!("run {cmd}")));
    }
    let words = shlex_split(cmd)?;
    if command.rtk_native {
        if let Some(summary) = native_rtk_summary(&command, &words, cwd) {
            return Some(summary);
        }
    }
    classify_command(cmd, &words, cwd).or_else(|| command.has_rtk.then(|| format!("run {cmd}")))
}

fn classify_command(cmd: &str, words: &[String], cwd: Option<&str>) -> Option<String> {
    let program = program_name(words.first()?);
    match program {
        "git" if git_action(words) == Some("diff") => Some("Δ Diff".to_string()),
        "diff" => Some("Δ Diff".to_string()),
        "apply_patch" | "patch" => Some("← Edit".to_string()),
        "rg" | "grep" => {
            parse_shell_search_summary(cmd, &words[0], cwd).or(Some("✱ Search".to_string()))
        }
        "semeja" => parse_shell_semeja_summary(cmd, cwd).or(Some("✱ Search".to_string())),
        "fd" | "find" => Some("✱ Search".to_string()),
        "git" if git_action(words) == Some("grep") => Some("✱ Search".to_string()),
        "cat" | "nl" | "bat" | "head" | "tail" => {
            let command = analyze_shell_command(cmd);
            Some(read_label(read_paths(&command).unwrap_or_default(), cwd))
        }
        "sed" => parse_shell_sed_summary(cmd, cwd).or(Some("✱ Search".to_string())),
        "ls" | "tree" => Some(
            format!("→ List {}", words[1..].join(" "))
                .trim_end()
                .to_string(),
        ),
        _ => runner_summary(words),
    }
}

fn native_rtk_summary(
    command: &ShellCommand,
    words: &[String],
    cwd: Option<&str>,
) -> Option<String> {
    let program = program_name(words.first()?);
    match program {
        "read" => Some(read_label(read_paths(command).unwrap_or_default(), cwd)),
        "smart" => Some(
            format!("→ Inspect {}", words[1..].join(" "))
                .trim_end()
                .to_string(),
        ),
        "test" => Some(
            format!(
                "Test {}",
                normalize_shell_command(command.text.strip_prefix(&words[0])?.trim())
            )
            .trim_end()
            .to_string(),
        ),
        "err" => Some(format!(
            "Run {} (errors only)",
            normalize_shell_command(command.text.strip_prefix(&words[0])?.trim())
        )),
        "summary" => Some(format!("Run {} (RTK summary)", words[1..].join(" "))),
        "git" if git_action(words) != Some("diff") && git_action(words) != Some("grep") => {
            Some(format!("Git {}", words[1..].join(" ")))
        }
        "gain" | "init" | "config" | "discover" | "session" | "telemetry" | "rewrite" | "hook"
        | "trust" | "untrust" | "verify" | "pipe" | "env" | "deps" => {
            Some(format!("RTK {}", command.text))
        }
        _ => None,
    }
}

fn read_label(paths: Vec<String>, cwd: Option<&str>) -> String {
    let paths = paths
        .iter()
        .map(|path| compact_command_path(path, cwd))
        .collect::<Vec<_>>()
        .join(", ");
    format!("→ Read {paths}").trim_end().to_string()
}

fn runner_summary(words: &[String]) -> Option<String> {
    let program = program_name(words.first()?);
    let action = words.get(1).map(String::as_str).unwrap_or("");
    let label = match (program, action) {
        ("cargo" | "go" | "npm" | "pnpm" | "yarn" | "dotnet" | "mvn" | "gradlew", "test") => "Test",
        ("pytest" | "jest" | "vitest" | "rspec" | "phpunit" | "pest", _) => "Test",
        ("cargo" | "go" | "npm" | "pnpm" | "dotnet", "build") => "Build",
        ("cargo", "check" | "clippy" | "fmt")
        | ("ruff", "check" | "format")
        | ("tsc" | "lint" | "mypy" | "prettier", _) => "Check",
        _ => return None,
    };
    Some(format!("{label} {}", words.join(" ")))
}

fn git_action(words: &[String]) -> Option<&str> {
    let mut index = 1;
    while let Some(word) = words.get(index) {
        index += 1;
        match word.as_str() {
            "-C" | "-c" | "--git-dir" | "--work-tree" => index += 1,
            _ if word.starts_with('-') => {}
            _ => return Some(word),
        }
    }
    None
}
