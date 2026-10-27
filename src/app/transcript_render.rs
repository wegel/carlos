//! Transcript block building, counting, and separator insertion orchestration.

use super::models::{Message, MessageKind, RenderedLine, Role};
use super::tool_summary::{summary_line_count, summary_lines, tool_failed};
pub(super) use super::transcript_diff::append_wrapped_diff_lines;
use super::transcript_diff::count_wrapped_diff_lines;
use super::transcript_source::{append_source_lines, count_source_lines};
pub(super) use super::transcript_spacing::should_insert_separator_between;
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

// --- Read Summaries ---

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

// --- Transcript Build ---

#[cfg(test)]
pub(super) fn build_rendered_lines(messages: &[Message], width: usize) -> Vec<RenderedLine> {
    build_rendered_lines_with_hidden(messages, width, None)
}

#[cfg(test)]
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

// --- Block Counting ---

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
        if should_insert_separator_between(prev, msg, width) {
            count += 1;
        }
    }

    count
        + tool_header_rows(msg, width)
        + match msg.kind {
            MessageKind::ToolSummary { rows } => summary_line_count(msg, width, rows),
            MessageKind::Diff => {
                count_wrapped_diff_lines(msg.file_path.as_deref(), &msg.text, width)
            }
            MessageKind::Plain => match msg.role {
                Role::ToolCall => count_tool_call_lines(msg, width),
                Role::Assistant | Role::Reasoning => {
                    count_wrapped_markdown_lines(msg.role, &msg.text, width)
                }
                Role::ToolOutput => {
                    if msg.file_path.is_some() {
                        if let Some(lines) = count_source_lines(&msg.text, width) {
                            return count + tool_header_rows(msg, width) + lines;
                        }
                    }
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
        if should_insert_separator_between(prev, msg, width) {
            count += 1;
        }
    }

    count
        + tool_header_rows(msg, width)
        + match msg.kind {
            MessageKind::ToolSummary { rows } => summary_line_count(msg, width, rows),
            MessageKind::Diff => {
                count_wrapped_diff_lines(msg.file_path.as_deref(), &msg.text, width)
            }
            MessageKind::Plain => match msg.role {
                Role::ToolCall => count_tool_call_lines(msg, width),
                Role::Assistant | Role::Reasoning => {
                    count_wrapped_markdown_lines(msg.role, &msg.text, width)
                }
                Role::ToolOutput => {
                    if msg.file_path.is_some() {
                        if let Some(lines) = count_source_lines(&msg.text, width) {
                            return count + tool_header_rows(msg, width) + lines;
                        }
                    }
                    count_wrapped_ansi_lines_cached(cache, msg.role, &msg.text, width)
                }
                _ => count_wrapped_message_lines_cached(cache, msg.role, &msg.text, width),
            },
        }
}

// --- Block Rendering ---

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
        if should_insert_separator_between(prev, msg, width) {
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

    if tool_header_rows(msg, width) > 0 {
        out.extend(summary_lines(msg, width, 0));
    }
    match msg.kind {
        MessageKind::ToolSummary { rows } => {
            if width >= 8 {
                out.extend(summary_lines(msg, width, rows));
            }
        }
        MessageKind::Diff => {
            append_wrapped_diff_lines(out, msg.role, msg.file_path.as_deref(), &msg.text, width)
        }
        MessageKind::Plain => match msg.role {
            Role::ToolCall => append_tool_call_lines(out, msg, width),
            Role::Assistant | Role::Reasoning => {
                append_wrapped_markdown_lines(out, msg.role, &msg.text, width);
            }
            Role::ToolOutput => {
                if !msg
                    .file_path
                    .as_deref()
                    .is_some_and(|path| append_source_lines(out, path, &msg.text, width))
                {
                    append_wrapped_ansi_lines(out, msg.role, &msg.text, width);
                }
            }
            _ => append_wrapped_message_lines(out, msg.role, &msg.text, width),
        },
    }
}

// --- Visibility Rules ---

fn message_has_visible_content(msg: &Message) -> bool {
    if msg.role == Role::ToolCall && msg.tool_output.is_some() {
        return true;
    }
    match msg.kind {
        MessageKind::Diff => !msg.text.trim().is_empty(),
        MessageKind::Plain | MessageKind::ToolSummary { .. } => !msg.text.trim().is_empty(),
    }
}

/// Count label rows shown before a full tool-output body.
pub(super) fn tool_header_rows(msg: &Message, width: usize) -> usize {
    if msg.role == Role::ToolOutput
        && (tool_failed(msg) || msg.tool_output.as_ref().is_some_and(|tool| tool.generated))
    {
        summary_line_count(msg, width, 0)
    } else {
        0
    }
}

fn count_tool_call_lines(msg: &Message, width: usize) -> usize {
    summary_line_count(msg, width, 0)
        + msg.text.split_once('\n').map_or(0, |(_, body)| {
            super::transcript_styles::count_wrapped_message_lines(msg.role, body, width)
        })
}

fn append_tool_call_lines(out: &mut Vec<RenderedLine>, msg: &Message, width: usize) {
    if width < 8 {
        return;
    }
    out.extend(summary_lines(msg, width, 0));
    if let Some((_, body)) = msg.text.split_once('\n') {
        append_wrapped_message_lines(out, msg.role, body, width);
    }
}
