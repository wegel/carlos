//! Paint native task panels from local display lines without borrowing a session or backend.

use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::render::{draw_str, fill_rect};
use super::task_panel_rows::PanelRows;
use super::view_models::TerminalSize;
use crate::theme::{COLOR_DIM, COLOR_PRIMARY, COLOR_STEP2, COLOR_TEXT};

/// Keep the selected target above independently scrolled detail text and controls.
pub(super) struct TaskPanel<'a> {
    pub(super) title: &'a str,
    pub(super) title_style: Style,
    pub(super) selected: Option<&'a str>,
    pub(super) body: &'a PanelRows,
    pub(super) scroll: usize,
    pub(super) footer: &'a str,
    pub(super) compact_help: Option<&'a str>,
}

/// Draw the same task frame for embedded and daemon consumers of native task facts.
pub(super) fn draw(buffer: &mut Buffer, panel: TaskPanel<'_>, size: TerminalSize) {
    fill_rect(
        buffer,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP2),
    );
    if size.width == 0 || size.height == 0 {
        return;
    }
    draw_str(buffer, 0, 0, panel.title, panel.title_style, size.width);
    if panel.selected.is_some() && size.height < 3 {
        return;
    }
    let header_rows = 1 + usize::from(panel.selected.is_some());
    if let Some(selected) = panel.selected {
        draw_str(
            buffer,
            0,
            1,
            selected,
            Style::default().fg(COLOR_PRIMARY),
            size.width,
        );
    }
    draw_body(buffer, panel, size, header_rows);
}

fn draw_body(buffer: &mut Buffer, panel: TaskPanel<'_>, size: TerminalSize, header_rows: usize) {
    let help = panel
        .compact_help
        .filter(|_| size.width < 40 && size.height >= 4);
    let height = size
        .height
        .saturating_sub(header_rows + 1 + usize::from(help.is_some()));
    debug_assert_eq!(panel.body.width(), size.width.max(1));
    for (row, line) in panel.body.window(panel.scroll, height).iter().enumerate() {
        draw_str(
            buffer,
            0,
            row + header_rows,
            line,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
    if let Some(help) = help {
        draw_str(
            buffer,
            0,
            size.height - 2,
            help,
            Style::default().fg(COLOR_DIM),
            size.width,
        );
    }
    draw_str(
        buffer,
        0,
        size.height - 1,
        panel.footer,
        Style::default().fg(COLOR_DIM),
        size.width,
    );
}
