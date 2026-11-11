//! Recognize one literal leading directory guard without interpreting the remaining shell program.

/// A proven literal directory prefix and its untouched command body.
pub(super) struct DirectoryPrefix<'a> {
    pub(super) directory: String,
    pub(super) body: &'a str,
}

/// Decode only a literal, argument-free shell launcher before checking its program.
pub(super) fn literal_shell_payload(text: &str) -> Option<String> {
    let (program, tail) = literal_word(horizontal(text))?;
    if !matches!(program.rsplit('/').next()?, "sh" | "bash" | "zsh" | "dash") {
        return None;
    }
    let (option, tail) = literal_word(horizontal(tail))?;
    if !matches!(option.as_str(), "-c" | "-lc" | "-cl") {
        return None;
    }
    let (script, tail) = literal_word(horizontal(tail))?;
    horizontal(tail).is_empty().then_some(script)
}

/// Match only `cd [--] literal && body`, preserving every byte inside the body.
pub(super) fn directory_prefix(text: &str) -> Option<DirectoryPrefix<'_>> {
    let (program, tail) = literal_word(horizontal(text))?;
    if program != "cd" {
        return None;
    }
    let (mut directory, mut tail) = literal_word(horizontal(tail))?;
    let options_ended = directory == "--";
    if options_ended {
        (directory, tail) = literal_word(horizontal(tail))?;
    }
    if directory.is_empty()
        || directory == "-"
        || !options_ended && directory.starts_with('-')
        || directory.chars().any(char::is_control)
    {
        return None;
    }
    let body = horizontal(tail)
        .strip_prefix("&&")?
        .trim_start_matches([' ', '\t', '\n']);
    if body.is_empty() || body.starts_with(['&', '|', ';', ')', '<', '>']) {
        return None;
    }
    Some(DirectoryPrefix { directory, body })
}

fn horizontal(mut text: &str) -> &str {
    loop {
        text = text.trim_start_matches([' ', '\t']);
        if let Some(tail) = text.strip_prefix("\\\n") {
            text = tail;
        } else {
            return text;
        }
    }
}

fn literal_word(text: &str) -> Option<(String, &str)> {
    let (mut quote, mut escaped) = (None, false);
    let mut end = text.len();
    for (index, ch) in text.char_indices() {
        if index > 8192 {
            return None;
        }
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, ch) {
            (Some('\''), '\'') | (Some('"'), '"') => quote = None,
            (Some('\''), _) => {}
            (Some('"'), '\\') | (None, '\\') => escaped = true,
            (Some('"'), '$' | '`') => return None,
            (Some('"'), _) => {}
            (None, '\'' | '"') => quote = Some(ch),
            (None, ch) if matches!(ch, ' ' | '\t' | '\n') || ";&|<>".contains(ch) => {
                end = index;
                break;
            }
            (None, '$' | '`' | '*' | '?' | '[' | ']' | '{' | '}' | '~' | '(' | ')') => {
                return None;
            }
            (None, '#') if index == 0 => return None,
            _ => {}
        }
    }
    if end == 0 || quote.is_some() || escaped {
        return None;
    }
    let words = shlex::split(&text[..end])?;
    let [word] = words.as_slice() else {
        return None;
    };
    Some((word.clone(), &text[end..]))
}
