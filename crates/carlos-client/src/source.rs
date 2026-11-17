//! Recognize existing numbered read prefixes without changing their bytes or inventing line positions.

/// Locate an existing boxed line-number prefix, including its original following space.
pub fn code_line_gutter_prefix_len(text: &str) -> Option<usize> {
    let mut chars = text.char_indices().peekable();

    while let Some((_, ch)) = chars.peek().copied() {
        if ch == ' ' {
            chars.next();
        } else {
            break;
        }
    }

    let mut saw_digit = false;
    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_digit {
        return None;
    }

    let mut saw_post_digit_space = false;
    while let Some((_, ch)) = chars.peek().copied() {
        if ch == ' ' {
            saw_post_digit_space = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_post_digit_space {
        return None;
    }

    let (_, bar) = chars.next()?;
    if bar != '│' {
        return None;
    }
    let (space_idx, space) = chars.next()?;
    if space != ' ' {
        return None;
    }
    Some(space_idx + space.len_utf8())
}

/// Borrow the existing native read prefix and untouched source portion of one line.
pub fn split_read_gutter(line: &str) -> (&str, &str) {
    if let Some(end) = code_line_gutter_prefix_len(line) {
        return (&line[..end], &line[end..]);
    }
    let trimmed = line.trim_start_matches(' ');
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return ("", line);
    }
    let rest = &trimmed[digits..];
    let prefix = if rest.starts_with('\t') || rest.starts_with('→') {
        rest.chars().next().map(char::len_utf8).unwrap_or(0)
    } else if rest.starts_with(':') {
        1
    } else {
        return ("", line);
    };
    let end = line.len() - rest.len() + prefix;
    (&line[..end], &line[end..])
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
