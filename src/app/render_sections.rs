//! Input area, overlay layering, and cursor placement for the main draw loop.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{draw_str, fill_rect};

use crate::app::render_input::InputLayout;
use crate::app::text::{split_at_cells, visual_width};

use crate::app::view_models::TerminalSize;
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_STEP2, COLOR_STEP8, COLOR_TEXT};

// --- Input area ---

/// Paint one local composer without reading or mutating a core session.
pub(in crate::app) fn draw_composer(
    buf: &mut Buffer,
    size: TerminalSize,
    input_layout: &InputLayout,
    content_x: usize,
    rewind: bool,
    name: Option<&str>,
) {
    let y0 = input_layout.input_top.saturating_sub(1);
    let gutter_color = if rewind {
        COLOR_DIFF_REMOVE
    } else {
        COLOR_GUTTER_USER
    };

    fill_rect(
        buf,
        0,
        y0,
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP2),
    );
    for row in 0..input_layout.input_height {
        let y = y0 + row;
        if content_x != 1 {
            draw_str(
                buf,
                0,
                y,
                "❯",
                Style::default()
                    .fg(gutter_color)
                    .add_modifier(Modifier::BOLD),
                1,
            );
        }
        if let Some(line) = input_layout.visible_lines.get(row) {
            draw_str(
                buf,
                content_x,
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }
    draw_session_placeholder(buf, input_layout, content_x, name);
}

fn draw_session_placeholder(
    buf: &mut Buffer,
    layout: &InputLayout,
    content_x: usize,
    name: Option<&str>,
) {
    let Some(name) = name else { return };
    let name = &name[..split_at_cells(name, layout.text_width)];
    let name_width = visual_width(name);
    if name_width <= layout.text_width {
        draw_str(
            buf,
            content_x + layout.text_width - name_width,
            layout.input_top.saturating_sub(1),
            name,
            Style::default().fg(COLOR_STEP8),
            layout.text_width,
        );
    }
}

// --- Overlays ---

// --- Cursor ---
