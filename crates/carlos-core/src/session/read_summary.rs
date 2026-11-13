//! Parse and combine source read labels without rendering.

use std::fmt::Write;

/// Read summary path.
pub(crate) fn read_summary_path(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix("→ Read")?.trim_start();
    if rest.is_empty() {
        return Some("");
    }
    // Directory hints can contain brackets; keep their complete identity.
    let path = if rest.contains(" (in ") {
        rest
    } else {
        rest.split_once(" [").map(|(p, _)| p).unwrap_or(rest)
    }
    .trim();
    Some(path)
}

/// Parse read summary.
pub fn parse_read_summary(text: &str) -> Option<(&str, usize)> {
    let path = read_summary_path(text)?;
    if let Some((base, count)) = path.rsplit_once(" ×") {
        let parsed = count.parse::<usize>().ok()?;
        return Some((base.trim_end(), parsed.max(1)));
    }
    Some((path, 1))
}

/// Format read summary with count.
pub fn format_read_summary_with_count(path: &str, count: usize) -> String {
    let mut summary = String::with_capacity(
        "→ Read".len()
            + path.len()
            + usize::from(!path.is_empty())
            + if count > 1 { 24 } else { 0 },
    );
    summary.push_str("→ Read");
    if !path.is_empty() {
        summary.push(' ');
        summary.push_str(path);
    }
    if count > 1 {
        write!(summary, " ×{count}").expect("writing to a String cannot fail");
    }
    summary
}
