//! Preserve complete native gutters and avoid numbering unmarked source lines.

use crate::source::split_read_gutter;

#[test]
fn numbered_read_prefixes_keep_the_original_source_bytes() {
    for (line, prefix) in [
        (" 42 │ let value = 7;", " 42 │ "),
        ("42→hello 🌍", "42→"),
        ("  42\tcode", "  42\t"),
        ("42:code", "42:"),
        ("42 code", ""),
        ("let x = 42;", ""),
    ] {
        let (gutter, source) = split_read_gutter(line);
        assert_eq!(gutter, prefix);
        assert_eq!(format!("{gutter}{source}"), line);
    }
}
