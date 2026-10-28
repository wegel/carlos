//! Paint transcript blocks with the 3c glyphs, continuation bars, and active step.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::flow_transcript::inline_reasoning;
use super::models::{Message, Role, TerminalSize};
use super::notifications::spinner_frame;
use super::render::{draw_rendered_line, draw_str, fill_rect};
use super::selection::compute_selection_range;
use super::state::AppState;
use super::text::visual_width;
use super::tool_summary::tool_gutter;
use super::MSG_TOP;
use crate::theme::{
    role_gutter_fg, role_gutter_symbol, role_row_bg, role_text_style, COLOR_PRIMARY,
};

/// Draw only visible rows while leaving cached text unchanged for selection.
pub(super) fn draw_transcript(buf: &mut Buffer, app: &AppState, height: usize, width: usize) {
    let active = active_step_index(app);
    let content_x = app.content_x(TerminalSize {
        width: buf.area.width as usize,
        height: buf.area.height as usize,
    });
    for row in 0..height {
        let index = app.viewport.scroll_top + row;
        draw_transcript_row(buf, app, index, MSG_TOP + row - 1, width, active, content_x);
    }
}

fn active_step_index(app: &AppState) -> Option<usize> {
    app.active_turn_id.as_ref()?;
    let start = app.turn_start_message_idx.unwrap_or(0);
    let index = app
        .item_to_message_index
        .iter()
        .filter(|(id, index)| **index >= start && !app.is_item_completed(id))
        .filter(|(_, index)| {
            app.messages
                .get(**index)
                .is_some_and(|m| !m.text.is_empty())
        })
        .map(|(_, index)| *index)
        .max()?;
    (app.messages[index].role == Role::Reasoning).then_some(index)
}

fn draw_transcript_row(
    buf: &mut Buffer,
    app: &AppState,
    index: usize,
    y: usize,
    width: usize,
    active: Option<usize>,
    content_x: usize,
) {
    let Some(line) = app.rendered_line_at(index) else {
        return;
    };
    let Some((message_index, row)) = app.render_cache.message_anchor(index) else {
        return;
    };
    let Some(message) = app.messages.get(message_index) else {
        return;
    };
    if line.separator {
        if app.layout.active.is_flow() {
            let active = heading_is_active(app, message_index, active, width);
            draw_section_header(buf, line, y, content_x, width, active);
        }
        return;
    }
    let first = row == app.render_cache.first_content_row(message_index);
    let active = active == Some(message_index);
    fill_message_row(buf, y, line.role);
    if !app.layout.active.is_flow() {
        draw_gutter(buf, message, y, first, active);
    }
    let selection = selected_cells(app, index, line.cells);
    draw_rendered_line(
        buf,
        content_x,
        y,
        width,
        line,
        role_text_style(line.role),
        selection,
    );
    if active {
        let last = app.render_cache.rendered_message_blocks[message_index]
            .as_ref()
            .is_some_and(|block| row + 1 == block.len());
        let inline = app.layout.active.is_flow() && inline_reasoning(message, width);
        paint_active_step(
            buf,
            y,
            width,
            &line.text,
            selection,
            (last, inline),
            content_x,
        );
    }
}

fn selected_cells(app: &AppState, index: usize, cells: usize) -> Option<(usize, usize)> {
    app.viewport
        .selection
        .and_then(|sel| compute_selection_range(sel, index, cells))
        .map(|(start, end)| (start.min(cells), end.min(cells)))
}

fn heading_is_active(app: &AppState, index: usize, active: Option<usize>, width: usize) -> bool {
    active.is_some_and(|active| {
        index <= active
            && app.messages[index..=active]
                .iter()
                .all(|message| message.role == Role::Reasoning && !inline_reasoning(message, width))
    })
}

fn fill_message_row(buf: &mut Buffer, y: usize, role: Role) {
    fill_rect(
        buf,
        0,
        y,
        buf.area.width as usize,
        1,
        Style::default().bg(role_row_bg(role)),
    );
}

fn draw_section_header(
    buf: &mut Buffer,
    line: &super::models::RenderedLine,
    y: usize,
    x: usize,
    width: usize,
    active: bool,
) {
    let text = if active {
        line.text.replacen("◇", "◆", 1)
    } else {
        line.text.clone()
    };
    let style = line
        .styled_segments
        .first()
        .map_or(Style::default(), |segment| segment.style);
    draw_str(buf, x, y, &text, style, width);
}

fn draw_gutter(buf: &mut Buffer, message: &Message, y: usize, first: bool, active: bool) {
    let (icon, color) = if active {
        ("◆", COLOR_PRIMARY)
    } else if matches!(message.role, Role::ToolCall | Role::ToolOutput) {
        tool_gutter(message)
    } else {
        (
            role_gutter_symbol(message.role),
            role_gutter_fg(message.role),
        )
    };
    let symbol = if first {
        icon
    } else if message.role == Role::Assistant {
        " "
    } else {
        "┃"
    };
    draw_str(
        buf,
        0,
        y,
        symbol,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
        2,
    );
}

fn paint_active_step(
    buf: &mut Buffer,
    y: usize,
    width: usize,
    text: &str,
    selection: Option<(usize, usize)>,
    markers: (bool, bool),
    content_x: usize,
) {
    let (last, inline) = markers;
    let cells = visual_width(text.trim_end()).min(width);
    for x in content_x..content_x + cells {
        let cell = &mut buf[(x as u16, y as u16)];
        cell.modifier.remove(Modifier::ITALIC | Modifier::DIM);
        cell.modifier.insert(Modifier::BOLD);
        if !selection.is_some_and(|(start, end)| (start..end).contains(&(x - content_x))) {
            cell.fg = COLOR_PRIMARY;
        }
    }
    if inline {
        buf[(content_x as u16, y as u16)].set_symbol("◆");
    }
    if last && width > 0 {
        draw_str(
            buf,
            (content_x + cells + 1).min(buf.area.right().saturating_sub(1) as usize),
            y,
            spinner_frame(),
            Style::default().fg(COLOR_PRIMARY),
            1,
        );
    }
}
