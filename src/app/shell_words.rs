//! Read shell words without executing them or changing quoted command tails.

/// Decode the first shell word and leave the rest of the command unchanged.
pub(super) fn next_word(text: &str) -> Option<(String, &str)> {
    let text = text.trim_start();
    let mut quote = None;
    let mut escaped = false;
    let mut end = text.len();
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('\'') if ch == '\'' => quote = None,
            Some('"') if ch == '"' => quote = None,
            Some('"') if ch == '\\' => escaped = true,
            Some(_) => {}
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '\\' => escaped = true,
            None if ch.is_whitespace() || ";&|<>".contains(ch) => {
                end = index;
                break;
            }
            None => {}
        }
    }
    if end == 0 || quote.is_some() || escaped {
        return None;
    }
    let mut words = shlex::split(&text[..end])?.into_iter();
    let word = words.next()?;
    if words.next().is_some() {
        return None;
    }
    Some((word, text[end..].trim_start()))
}

/// Detect operators outside quotes before treating a command as a single read.
pub(super) fn has_shell_operators(text: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    for ch in text.trim().chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match quote {
            Some('\'') if ch == '\'' => quote = None,
            Some('"') if ch == '"' => quote = None,
            Some('"') if ch == '\\' => escaped = true,
            Some(_) => {}
            None if ch == '\'' || ch == '"' => quote = Some(ch),
            None if ch == '\\' => escaped = true,
            None if ";&|<>\n\r`".contains(ch) => return true,
            None => {}
        }
    }
    quote.is_some() || escaped
}

/// Recognize an environment assignment in a shell prefix.
pub(super) fn is_assignment(word: &str) -> bool {
    let Some((name, _)) = word.split_once('=') else {
        return false;
    };
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

/// Match an executable by filename, including quoted or absolute paths.
pub(super) fn program_name(program: &str) -> &str {
    program.rsplit('/').next().unwrap_or(program)
}
