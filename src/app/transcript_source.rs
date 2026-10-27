//! Color visible file reads while leaving command labels and line numbers intact.

use ratatui::style::Style;
use serde_json::Value;

use super::models::{RenderedLine, Role, StyledSegment};
use super::shell_read::single_source_path;
use super::source_highlight::SourceHighlighter;
use super::text::wrap_natural_count_by_cells;
use super::tools::{first_i64_at_paths, first_string_at_paths, tool_command, tool_name};
use super::transcript_styles::append_wrapped_styled_logical_lines;
use crate::theme::COLOR_STEP8;

/// Read a source path only from a successful read tool or a single read action.
pub(super) fn source_path_from_item(item: &Value) -> Option<String> {
    if first_i64_at_paths(item, &[&["exitCode"], &["exit_code"]]).unwrap_or(0) != 0
        || item.get("isError").and_then(Value::as_bool) == Some(true)
        || item.get("is_error").and_then(Value::as_bool) == Some(true)
        || first_string_at_paths(item, &[&["stderr"], &["metadata", "stderr"]])
            .is_some_and(|text| !text.trim().is_empty())
    {
        return None;
    }
    if tool_name(item).is_some_and(|name| name.eq_ignore_ascii_case("read")) {
        return first_string_at_paths(
            item,
            &[
                &["input", "filePath"],
                &["input", "file_path"],
                &["input", "path"],
                &["filePath"],
                &["path"],
            ],
        );
    }
    if let Some(actions) = item.get("commandActions").and_then(Value::as_array) {
        if actions.len() > 1 {
            return None;
        }
        if let Some(action) = actions
            .first()
            .filter(|action| action.get("type").and_then(Value::as_str) == Some("read"))
        {
            if let Some(path) = action.get("path").and_then(Value::as_str) {
                return Some(path.to_string());
            }
        }
    }
    single_source_path(&tool_command(item)?)
}

/// Color the saved read output, keeping terminal colors when a tool supplied them.
pub(super) fn append_source_lines(
    out: &mut Vec<RenderedLine>,
    path: &str,
    text: &str,
    width: usize,
) -> bool {
    if text.contains('\u{1b}') || text.contains('\u{009b}') {
        return false;
    }
    let mut highlighter = SourceHighlighter::for_path(path);
    let lines = text
        .split('\n')
        .map(|line| {
            if line.starts_with("$ ") || line.starts_with("exit code: ") {
                return vec![StyledSegment {
                    text: line.to_string(),
                    style: Style::default(),
                }];
            }
            let (gutter, source) = split_read_gutter(line);
            let mut segments = Vec::new();
            if !gutter.is_empty() {
                segments.push(StyledSegment {
                    text: gutter.to_string(),
                    style: Style::default().fg(COLOR_STEP8),
                });
            }
            if let Some(highlighter) = highlighter.as_mut() {
                segments.extend(highlighter.highlight(source, Style::default()));
            } else {
                segments.push(StyledSegment {
                    text: source.to_string(),
                    style: Style::default(),
                });
            }
            segments
        })
        .collect();
    append_wrapped_styled_logical_lines(out, Role::ToolOutput, lines, width);
    true
}

/// Count source rows without parsing tokens or interpreting Markdown fences.
pub(super) fn count_source_lines(text: &str, width: usize) -> Option<usize> {
    if text.contains('\u{1b}') || text.contains('\u{009b}') {
        return None;
    }
    if width < 8 {
        return Some(0);
    }
    Some(
        text.split('\n')
            .map(|line| {
                if line.is_empty() {
                    1
                } else {
                    wrap_natural_count_by_cells(line, width)
                }
            })
            .sum(),
    )
}

fn split_read_gutter(line: &str) -> (&str, &str) {
    if let Some(end) = super::transcript_styles::code_line_gutter_prefix_len(line) {
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
