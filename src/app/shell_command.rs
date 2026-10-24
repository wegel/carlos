//! Inspect shell and RTK wrappers while keeping the displayed command separate.

use super::shell_words::{is_assignment, next_word, program_name};

/// The command Carlos can classify after unwrapping launchers and RTK prefixes.
pub(super) struct ShellCommand {
    pub(super) text: String,
    pub(super) rtk_native: bool,
    pub(super) has_rtk: bool,
}

/// Remove only an outer shell invocation for display; preserve RTK and its flags.
pub(super) fn display_shell_command(raw: &str) -> String {
    shell_payload(raw).unwrap_or_else(|| raw.trim().to_string())
}

/// Inspect wrappers without evaluating variables, substitutions, or script bodies.
pub(super) fn analyze_shell_command(raw: &str) -> ShellCommand {
    let mut text = raw.trim().to_string();
    let mut has_rtk = false;
    for _ in 0..32 {
        if let Some(payload) = launcher_payload(&text) {
            text = payload;
            continue;
        }
        let Some((program, tail)) = next_word(&text) else {
            break;
        };
        if program_name(&program) != "rtk" {
            break;
        }
        has_rtk = true;
        let Some(tail) = skip_rtk_flags(tail) else {
            break;
        };
        let Some((subcommand, inner)) = next_word(tail) else {
            break;
        };
        match subcommand.as_str() {
            "proxy" | "bypass" => {
                let Some(command) = skip_rtk_flags(inner) else {
                    break;
                };
                text = command.to_string();
            }
            "run" => {
                let Some(command) = rtk_run_command(inner) else {
                    break;
                };
                text = command;
            }
            _ => {
                return ShellCommand {
                    text: tail.to_string(),
                    rtk_native: true,
                    has_rtk,
                }
            }
        }
    }
    ShellCommand {
        text,
        rtk_native: false,
        has_rtk,
    }
}

fn launcher_payload(text: &str) -> Option<String> {
    if let Some(payload) = shell_payload(text) {
        return Some(payload);
    }
    let (program, tail) = next_word(text)?;
    if is_assignment(&program) {
        return Some(tail.to_string());
    }
    match program_name(&program) {
        "env" => env_command(tail).map(str::to_string),
        "command" | "exec" => Some(tail.strip_prefix("-- ").unwrap_or(tail).to_string()),
        _ => None,
    }
}

fn skip_rtk_flags(mut text: &str) -> Option<&str> {
    loop {
        let (word, tail) = next_word(text)?;
        if word == "--" {
            return Some(tail);
        }
        if word == "--ultra-compact"
            || word == "--skip-env"
            || word == "--verbose"
            || word == "-u"
            || (word.starts_with('-') && word.len() > 1 && word[1..].chars().all(|ch| ch == 'v'))
        {
            text = tail;
            continue;
        }
        if word.starts_with('-') {
            return None;
        }
        return Some(text.trim_start());
    }
}

fn shell_payload(text: &str) -> Option<String> {
    let (program, mut tail) = next_word(text)?;
    if !matches!(program_name(&program), "sh" | "bash" | "zsh" | "dash") {
        return None;
    }
    loop {
        let (option, rest) = next_word(tail)?;
        if matches!(option.as_str(), "-o" | "-O") {
            tail = next_word(rest)?.1;
            continue;
        }
        if !option.starts_with('-') || option.starts_with("--") {
            return None;
        }
        if option[1..].contains('c') {
            let (script, args) = next_word(rest)?;
            return args.is_empty().then_some(script);
        }
        tail = rest;
    }
}

fn env_command(mut text: &str) -> Option<&str> {
    loop {
        let (word, tail) = next_word(text)?;
        match word.as_str() {
            "--" => return Some(tail),
            "-i" | "--ignore-environment" => text = tail,
            "-u" | "--unset" => text = next_word(tail)?.1,
            _ if word.starts_with("--unset=") || is_assignment(&word) => text = tail,
            _ if word.starts_with('-') => return None,
            _ => return Some(text),
        }
    }
}

fn rtk_run_command(text: &str) -> Option<String> {
    let text = skip_rtk_run_flags(text)?;
    let (word, tail) = next_word(text)?;
    if matches!(word.as_str(), "-c" | "--command") {
        let (script, rest) = next_word(tail)?;
        return rest.is_empty().then_some(script);
    }
    if let Some(script) = word
        .strip_prefix("--command=")
        .or_else(|| word.strip_prefix("-c"))
    {
        return tail.is_empty().then(|| script.to_string());
    }
    // RTK joins positional argv with spaces before handing it to sh -c.
    Some(shlex::split(text)?.join(" "))
}

fn skip_rtk_run_flags(mut text: &str) -> Option<&str> {
    loop {
        let (word, tail) = next_word(text)?;
        match word.as_str() {
            "--ultra-compact" | "--skip-env" => text = tail,
            "--" => return Some(tail),
            _ => return Some(text),
        }
    }
}
