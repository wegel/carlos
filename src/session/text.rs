//! Escape native prose and labels without terminal dependencies.

/// Escape terminal controls and directional overrides while keeping prose line breaks.
pub(crate) fn safe_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if (ch.is_control() && ch != '\n')
                || matches!(ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            {
                ch.escape_default().to_string()
            } else {
                ch.to_string()
            }
        })
        .collect()
}

/// Escape line breaks as well as controls in labels, IDs, paths, and server names.
pub(crate) fn safe_label(value: &str) -> String {
    safe_text(value).replace('\n', "\\n")
}
