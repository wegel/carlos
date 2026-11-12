//! Parse and combine source read labels without rendering.

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
pub(crate) fn parse_read_summary(text: &str) -> Option<(&str, usize)> {
    let path = read_summary_path(text)?;
    if let Some((base, count)) = path.rsplit_once(" ×") {
        let parsed = count.parse::<usize>().ok()?;
        return Some((base.trim_end(), parsed.max(1)));
    }
    Some((path, 1))
}

/// Format read summary with count.
pub(crate) fn format_read_summary_with_count(path: &str, count: usize) -> String {
    let base = if path.is_empty() {
        "→ Read".to_string()
    } else {
        format!("→ Read {path}")
    };
    if count > 1 {
        format!("{base} ×{count}")
    } else {
        base
    }
}
