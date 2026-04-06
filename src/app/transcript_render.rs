use super::models::{Message, MessageKind, RenderedLine, Role, TerminalSize};
pub(super) use super::transcript_diff::append_wrapped_diff_lines;
use super::transcript_diff::count_wrapped_diff_lines;
#[cfg(test)]
pub(super) use super::transcript_styles::normalize_styled_segments_for_part;
pub(super) use super::transcript_styles::{
    append_wrapped_ansi_lines, append_wrapped_markdown_lines, append_wrapped_message_lines,
    RenderCountCache,
};
use super::transcript_styles::{
    count_wrapped_ansi_lines_cached, count_wrapped_markdown_lines,
    count_wrapped_message_lines_cached,
};
use super::MSG_CONTENT_X;

pub(super) fn read_summary_path(text: &str) -> Option<&str> {
    let trimmed = text.trim();
    let rest = trimmed.strip_prefix("→ Read")?.trim_start();
    if rest.is_empty() {
        return Some("");
    }
    let path = rest.split_once(" [").map(|(p, _)| p).unwrap_or(rest).trim();
    Some(path)
}

pub(super) fn parse_read_summary(text: &str) -> Option<(&str, usize)> {
    let path = read_summary_path(text)?;
    if let Some((base, count)) = path.rsplit_once(" ×") {
        let parsed = count.parse::<usize>().ok()?;
        return Some((base.trim_end(), parsed.max(1)));
    }
    Some((path, 1))
}

pub(super) fn format_read_summary_with_count(path: &str, count: usize) -> String {
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

#[cfg(test)]
pub(super) fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    build_rendered_lines_with_hidden(messages, width, None)
}

pub(super) fn build_rendered_lines_with_hidden(
    messages: &[Message],
    width: usize,
    hidden_user_message_idx: Option<usize>,
) -> Vec<RenderedLine> {
    let mut out = Vec::new();
    let mut previous_visible: Option<&Message> = None;

    for (idx, msg) in messages.iter().enumerate() {
        if hidden_user_message_idx == Some(idx) && msg.role == Role::User {
            continue;
        }
        if !message_has_visible_content(msg) {
            continue;
        }
        append_rendered_block_for_message(&mut out, previous_visible, msg, width);
        previous_visible = Some(msg);
    }

    out
}

pub(super) fn build_rendered_block_for_message(
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) -> Vec<RenderedLine> {
    let mut out = Vec::new();
    append_rendered_block_for_message(&mut out, previous_visible, msg, width);
    out
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn count_rendered_block_for_message(
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) -> usize {
    if !message_has_visible_content(msg) {
        return 0;
    }

    let mut count = 0usize;
    if let Some(prev) = previous_visible {
        if should_insert_separator_between(prev, msg) {
            count += 1;
        }
    }

    count
        + match msg.kind {
            MessageKind::Diff => {
                count_wrapped_diff_lines(msg.file_path.as_deref(), &msg.text, width)
            }
            MessageKind::Plain => match msg.role {
                Role::Assistant | Role::Reasoning => {
                    count_wrapped_markdown_lines(msg.role, &msg.text, width)
                }
                Role::ToolOutput => {
                    super::transcript_styles::count_wrapped_ansi_lines(msg.role, &msg.text, width)
                }
                _ => super::transcript_styles::count_wrapped_message_lines(
                    msg.role, &msg.text, width,
                ),
            },
        }
}

pub(super) fn count_rendered_block_for_message_cached<'a>(
    cache: &mut RenderCountCache<'a>,
    previous_visible: Option<&Message>,
    msg: &'a Message,
    width: usize,
) -> usize {
    if !message_has_visible_content(msg) {
        return 0;
    }

    let mut count = 0usize;
    if let Some(prev) = previous_visible {
        if should_insert_separator_between(prev, msg) {
            count += 1;
        }
    }

    count
        + match msg.kind {
            MessageKind::Diff => {
                count_wrapped_diff_lines(msg.file_path.as_deref(), &msg.text, width)
            }
            MessageKind::Plain => match msg.role {
                Role::Assistant | Role::Reasoning => {
                    count_wrapped_markdown_lines(msg.role, &msg.text, width)
                }
                Role::ToolOutput => {
                    count_wrapped_ansi_lines_cached(cache, msg.role, &msg.text, width)
                }
                _ => count_wrapped_message_lines_cached(cache, msg.role, &msg.text, width),
            },
        }
}

pub(super) fn append_rendered_block_for_message(
    out: &mut Vec<RenderedLine>,
    previous_visible: Option<&Message>,
    msg: &Message,
    width: usize,
) {
    if !message_has_visible_content(msg) {
        return;
    }
    if let Some(prev) = previous_visible {
        if should_insert_separator_between(prev, msg) {
            out.push(RenderedLine {
                text: String::new(),
                styled_segments: Vec::new(),
                role: Role::System,
                separator: true,
                cells: 0,
                soft_wrap_to_next: false,
            });
        }
    }

    match msg.kind {
        MessageKind::Diff => {
            append_wrapped_diff_lines(out, msg.role, msg.file_path.as_deref(), &msg.text, width)
        }
        MessageKind::Plain => match msg.role {
            Role::Assistant | Role::Reasoning => {
                append_wrapped_markdown_lines(out, msg.role, &msg.text, width);
            }
            Role::ToolOutput => append_wrapped_ansi_lines(out, msg.role, &msg.text, width),
            _ => append_wrapped_message_lines(out, msg.role, &msg.text, width),
        },
    }
}

fn message_has_visible_content(msg: &Message) -> bool {
    match msg.kind {
        MessageKind::Diff => !msg.text.trim().is_empty(),
        MessageKind::Plain => !msg.text.trim().is_empty(),
    }
}

fn should_insert_separator_between(prev: &Message, next: &Message) -> bool {
    let _ = (prev, next);
    true
}

pub(super) fn transcript_content_width(size: TerminalSize) -> usize {
    size.width.saturating_sub(MSG_CONTENT_X + 1)
}
