//! Edit native queued messages in a separate panel with a pinned selected identity.

use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui_textarea::TextArea;

use super::render::{draw_str, fill_rect};
use super::text::{split_at_cells, visual_width, wrap_natural_by_cells};
use super::TerminalSize;
use crate::theme::{COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_TEXT};

/// Draw the same queue controls from either embedded or daemon facts.
pub(super) fn draw_panel(
    buf: &mut Buffer,
    editor: Option<&TextArea<'_>>,
    lines: Vec<String>,
    size: TerminalSize,
) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    let width = size.width.saturating_sub(2).max(1);
    let footer = footer_lines(editor.is_some(), size);
    let body_height = size
        .height
        .saturating_sub(footer.len() + 2 + usize::from(editor.is_some() && editable(size)));
    let body = lines
        .into_iter()
        .flat_map(|line| {
            wrap_natural_by_cells(&carlos_core::session::text::safe_text(&line), width)
        })
        .collect::<Vec<_>>();
    for (row, line) in body.iter().take(body_height).enumerate() {
        draw_str(
            buf,
            1,
            row,
            line,
            Style::default().fg(if row == 0 { COLOR_PRIMARY } else { COLOR_TEXT }),
            width,
        );
    }
    if let Some((text, _)) = editor
        .filter(|_| editable(size))
        .and_then(|editor| editor_line(Some(editor), width))
    {
        draw_str(
            buf,
            1,
            body_height,
            &text,
            Style::default().fg(COLOR_TEXT),
            width,
        );
    }
    for (index, line) in footer.iter().take(size.height).enumerate() {
        draw_str(
            buf,
            1,
            size.height.saturating_sub(footer.len()) + index,
            line,
            Style::default().fg(COLOR_DIM),
            width,
        );
    }
}

/// Keep a local queue editor's caret independent of the ordinary composer.
pub(super) fn cursor_for(editor: Option<&TextArea<'_>>, size: TerminalSize) -> (usize, usize) {
    if !editable(size) {
        return (0, 0);
    }
    let width = size.width.saturating_sub(2).max(1);
    let footer_rows = footer_lines(editor.is_some(), size).len();
    editor_line(editor, width).map_or((0, 0), |(_, column)| {
        (column + 1, size.height.saturating_sub(footer_rows + 3))
    })
}

fn editor_line(editor: Option<&TextArea<'_>>, width: usize) -> Option<(String, usize)> {
    let editor = editor?;
    let (row, column) = editor.cursor();
    let line = &editor.lines()[row];
    let prefix = line.chars().take(column).collect::<String>();
    let cells = visual_width(&prefix);
    let skip = cells.saturating_sub(width.saturating_sub(1));
    let start = split_at_cells(line, skip);
    Some((
        carlos_core::session::text::safe_text(&line[start..]),
        cells.saturating_sub(skip),
    ))
}

fn footer_lines(editing: bool, size: TerminalSize) -> Vec<String> {
    let text = if !editable(size) {
        "Esc back"
    } else if editing {
        "Ctrl+S save · Esc back"
    } else if size.width < 60 {
        "↑↓ select e edit d delete s start Alt+↑↓ move r refresh v draft F3 mode Esc"
    } else {
        "↑↓ select · e edit · d delete · s start · Alt+↑↓ reorder · r refresh · v recover draft · F3 mode · Esc back"
    };
    wrap_natural_by_cells(text, size.width.saturating_sub(2).max(1))
}

/// Report whether the terminal has room for queue edits.
pub(super) fn editable(size: TerminalSize) -> bool {
    size.width >= 22 && size.height >= 12
}
