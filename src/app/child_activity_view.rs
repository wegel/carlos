//! Paint the same bounded child activity from embedded records or checked daemon display scalars.

use std::borrow::Cow;

/// Reader facts stay separate from any client's selected row, scroll or composer.
pub(super) struct ActivityView<'a> {
    pub error: Option<&'a str>,
    pub known: bool,
    pub reading: bool,
    pub count: usize,
    pub older: bool,
    pub tail: &'a str,
}

/// One borrowed display record never exposes native control parameters to the painter.
pub(super) struct ActivityRecord<'a> {
    pub turn: &'a str,
    pub item: &'a str,
    pub kind: &'a str,
    pub text: Cow<'a, str>,
    pub complete: bool,
}

/// Keep the legacy item order, display limits, empty-state explanation and separate live tail.
pub(super) fn lines<'a>(
    view: ActivityView<'a>,
    records: impl Iterator<Item = ActivityRecord<'a>>,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(error) = view.error {
        lines.push(error.into());
    }
    lines.push(format!(
        "Read-only child activity: {} items retained (max128/8MiB)",
        view.count
    ));
    if view.reading {
        lines.push("Reading child items…".into());
    }
    if view.count == 0 && view.known && !view.reading {
        lines.push("No native item records returned. This does not mean no work ran; some native tools have no saved display record.".into());
    }
    if view.older {
        lines.push("Older activity exists: l loads more, r reloads latest".into());
    }
    if view.count >= 128 {
        lines.push("128-item view limit reached; r reloads latest".into());
    }
    let mut remaining = 128 * 1024;
    for record in records {
        if remaining == 0 {
            lines.push("Display limited to128KiB; native records remain retained".into());
            break;
        }
        lines.push(format!(
            "{} · {} · {}",
            record.turn, record.item, record.kind
        ));
        let text = bounded(&record.text, remaining.min(32 * 1024), record.complete);
        remaining = remaining.saturating_sub(text.len());
        lines.extend(text.lines().map(str::to_string));
    }
    if !view.tail.is_empty() {
        lines.push("Live output since the last native item snapshot (last16KiB):".into());
        lines.extend(view.tail.lines().map(str::to_string));
    }
    lines
}

fn bounded(value: &str, limit: usize, complete: bool) -> String {
    if value.len() <= limit && complete {
        return value.into();
    }
    let mut end = limit.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[Display limited; complete native record retained]",
        &value[..end]
    )
}
