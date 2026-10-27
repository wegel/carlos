//! Select substantial scripts and compound shell commands without executing them.

/// Classify the command after Carlos has removed launcher and RTK wrappers.
pub(crate) fn eligible(command: &str) -> bool {
    if command.len() > 131_072 {
        return false;
    }
    if command.trim().lines().count() > 1 {
        return true;
    }
    let Some(words) = shlex::split(command) else {
        return false;
    };
    let Some(program) = words.first().map(|s| basename(s)) else {
        return false;
    };
    if script_program(program) {
        if let Some(index) = words
            .iter()
            .position(|s| matches!(s.as_str(), "-c" | "-e" | "--eval"))
        {
            let payload = words.get(index + 1).map(String::as_str).unwrap_or("");
            return payload.chars().count() >= 200
                || payload.lines().filter(|l| !l.trim().is_empty()).count() >= 10;
        }
        if words.iter().skip(1).any(|word| script_path(word)) {
            return true;
        }
    }
    if script_path(words.first().map(String::as_str).unwrap_or("")) {
        return true;
    }
    command.chars().count() >= 200 && compound_commands(command) >= 2
}

/// Named scripts need source-aware snapshots; inline programs use command text.
pub(crate) fn file_based(command: &str) -> bool {
    if command
        .lines()
        .next()
        .is_some_and(|header| header.contains("<<"))
    {
        return false;
    }
    let Some(words) = shlex::split(command) else {
        return false;
    };
    if words
        .iter()
        .any(|w| matches!(w.as_str(), "-c" | "-e" | "--eval"))
    {
        return false;
    }
    words.first().is_some_and(|word| script_path(word))
        || words
            .first()
            .is_some_and(|word| script_program(basename(word)))
            && words.iter().skip(1).any(|word| script_path(word))
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

fn script_program(program: &str) -> bool {
    matches!(
        program,
        "python"
            | "python3"
            | "python3.12"
            | "python3.13"
            | "python3.14"
            | "node"
            | "ruby"
            | "perl"
            | "bash"
            | "sh"
            | "zsh"
            | "dash"
    )
}

fn script_path(word: &str) -> bool {
    !word.starts_with('-')
        && [".py", ".sh", ".bash", ".rb", ".pl", ".js", ".mjs"]
            .iter()
            .any(|suffix| word.ends_with(suffix))
}

/// Count shell stages while ignoring quoted operators, escapes, and comments.
fn compound_commands(text: &str) -> usize {
    let (mut quote, mut escaped, mut comment) = (None, false, false);
    let (mut stages, mut content) = (0, false);
    for ch in text.chars() {
        if comment && ch != '\n' {
            continue;
        }
        if escaped {
            escaped = false;
            content = true;
            continue;
        }
        match quote {
            Some('\'') if ch == '\'' => quote = None,
            Some('"') if ch == '"' => quote = None,
            Some('`') if ch == '`' => quote = None,
            Some('"') if ch == '\\' => escaped = true,
            Some(_) => {}
            None if ch == '#' && !content => comment = true,
            None if matches!(ch, '\'' | '"' | '`') => {
                quote = Some(ch);
                content = true;
            }
            None if ch == '\\' => escaped = true,
            None if matches!(ch, ';' | '&' | '|' | '\n') => {
                if content {
                    stages += 1;
                    content = false;
                }
                comment = false;
            }
            None if !ch.is_whitespace() => content = true,
            _ => {}
        }
    }
    if quote.is_some() || escaped {
        return 0;
    }
    stages + usize::from(content)
}
