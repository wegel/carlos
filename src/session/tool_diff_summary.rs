//! Count diff changes and split files without terminal styles or layout.

use super::models::Message;

/// Summarize all changed lines, naming one file or the number of files.
pub(crate) fn diff_summary(message: &Message) -> String {
    let added = message
        .text
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++ "))
        .count();
    let removed = message
        .text
        .lines()
        .filter(|line| line.starts_with('-') && !line.starts_with("--- "))
        .count();
    let files = split_file_diffs(&message.text);
    if files.len() > 1 {
        return format!("Δ Diff {} files (+{added}, -{removed})", files.len());
    }
    let path = message
        .file_path
        .as_deref()
        .or_else(|| {
            message
                .text
                .lines()
                .find_map(|line| {
                    line.strip_prefix("+++ ")
                        .filter(|path| *path != "/dev/null")
                })
                .map(|path| path.strip_prefix("b/").unwrap_or(path))
        })
        .or_else(|| {
            message
                .text
                .lines()
                .find_map(|line| line.strip_prefix("--- "))
                .map(|path| path.strip_prefix("a/").unwrap_or(path))
        })
        .unwrap_or("files");
    format!("Δ Diff {path} (+{added}, -{removed})")
}

/// Split a git diff before parsing so each file keeps its own name and language.
pub(crate) fn split_file_diffs(diff: &str) -> Vec<&str> {
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
