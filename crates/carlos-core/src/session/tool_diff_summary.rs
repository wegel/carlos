//! Count diff changes and split files without terminal styles or layout.

use std::borrow::Cow;

use crate::session::models::Message;
use crate::session::tools::strip_terminal_controls;

/// Summarize all changed lines, naming one file or the number of files.
pub fn diff_summary(message: &Message) -> String {
    diff_summary_parts(&message.text, message.file_path.as_deref())
}

/// Summarize borrowed diff text and its path without creating another source message.
pub fn diff_summary_parts(text: &str, file_path: Option<&str>) -> String {
    let (added, removed) = diff_counts(text);
    let files = split_file_diffs(text);
    if files.len() > 1 {
        return format!("Δ Diff {} files (+{added}, -{removed})", files.len());
    }
    let path = file_path
        .or_else(|| {
            text.lines()
                .find_map(|line| {
                    line.strip_prefix("+++ ")
                        .filter(|path| *path != "/dev/null")
                })
                .map(|path| path.strip_prefix("b/").unwrap_or(path))
        })
        .or_else(|| {
            text.lines()
                .find_map(|line| line.strip_prefix("--- "))
                .map(|path| path.strip_prefix("a/").unwrap_or(path))
        })
        .unwrap_or("files");
    format!("Δ Diff {path} (+{added}, -{removed})")
}

/// Count changed source lines without counting the two file headers.
pub(crate) fn diff_counts(text: &str) -> (u64, u64) {
    let text = if text.contains(['\u{1b}', '\u{9b}']) {
        Cow::Owned(strip_terminal_controls(text))
    } else {
        Cow::Borrowed(text)
    };
    let (mut added, mut removed) = (0, 0);
    let mut remaining = None;
    for line in text.lines() {
        if let Some(counts) = hunk_counts(line) {
            remaining = (counts != (0, 0)).then_some(counts);
            continue;
        }
        if line.starts_with("diff --git ") {
            remaining = None;
        }
        let addition = line.starts_with('+') && (remaining.is_some() || !line.starts_with("+++ "));
        let removal = line.starts_with('-') && (remaining.is_some() || !line.starts_with("--- "));
        added += u64::from(addition);
        removed += u64::from(removal);
        if let Some((old, new)) = &mut remaining {
            *old = old.saturating_sub(u64::from(removal || line.starts_with(' ')));
            *new = new.saturating_sub(u64::from(addition || line.starts_with(' ')));
            if *old == 0 && *new == 0 {
                remaining = None;
            }
        }
    }
    (added, removed)
}

fn hunk_counts(line: &str) -> Option<(u64, u64)> {
    let (old, new) = line.strip_prefix("@@ -")?.split_once(" +")?;
    let (new, _) = new.split_once(" @@")?;
    Some((range_count(old)?, range_count(new)?))
}

fn range_count(range: &str) -> Option<u64> {
    let (start, count) = range.split_once(',').unwrap_or((range, "1"));
    start.parse::<u64>().ok()?;
    count.parse().ok()
}

/// Split a git diff before parsing so each file keeps its own name and language.
pub fn split_file_diffs(diff: &str) -> Vec<&str> {
    let mut starts = vec![0];
    let mut offset = 0;
    for line in diff.split_inclusive('\n') {
        if offset > 0 && line.starts_with("diff --git ") {
            starts.push(offset);
        }
        offset += line.len();
    }
    starts.push(diff.len());
    starts
        .windows(2)
        .map(|pair| &diff[pair[0]..pair[1]])
        .collect()
}
