//! Paint transcript blocks with the 3c glyphs, continuation bars, and active step.

use carlos_core::session::models::Role;
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::flow_transcript::inline_reasoning;
use super::notifications::spinner_frame;
use super::render::{draw_rendered_line, draw_str, fill_rect};
use super::render_cache_state::{RenderCacheState, TranscriptSource};
use super::state::AppState;
use super::text::visual_width;
use super::tool_summary::tool_gutter;
use super::transcript_input::RenderInput;
use super::view_models::TerminalSize;
use super::viewport_state::ViewportState;
use super::MSG_TOP;
use crate::theme::{
    role_gutter_fg, role_gutter_symbol, role_row_bg, role_text_style, COLOR_PRIMARY,
};

/// Borrow only renderer facts and device-local viewport state, without a mutable session owner.
pub(super) struct TranscriptFrame<'a, S: TranscriptSource + ?Sized> {
    pub source: &'a S,
    pub cache: &'a RenderCacheState,
    pub viewport: &'a ViewportState,
    pub flow: bool,
    pub content_x: usize,
    pub active: Option<usize>,
}

/// Preserve the embedded caller until its separate deletion commit.
pub(super) fn draw_transcript(buf: &mut Buffer, app: &AppState, height: usize, width: usize) {
    let frame = TranscriptFrame {
        source: app.session.messages().entries(),
        cache: &app.view.render_cache,
        viewport: &app.view.viewport,
        flow: app.view.layout.active.is_flow(),
        content_x: app.content_x(TerminalSize {
            width: buf.area.width as usize,
            height: buf.area.height as usize,
        }),
        active: app.session.active_reasoning_step(),
    };
    draw_rows(buf, &frame, height, width);
}

/// Draw the same body rows for borrowed native or exported offline facts.
pub(super) fn draw_rows<S: TranscriptSource + ?Sized>(
    buf: &mut Buffer,
    frame: &TranscriptFrame<'_, S>,
    height: usize,
    width: usize,
) {
    for row in 0..height {
        let index = frame.viewport.scroll_top + row;
        draw_transcript_row(buf, frame, index, MSG_TOP + row - 1, width);
    }
}

fn draw_transcript_row<S: TranscriptSource + ?Sized>(
    buf: &mut Buffer,
    frame: &TranscriptFrame<'_, S>,
    index: usize,
    y: usize,
    width: usize,
) {
    let Some(line) = frame.cache.rendered_line_at(index) else {
        return;
    };
    let Some((message_index, row)) = frame.cache.message_anchor(index) else {
        return;
    };
    let Some(message) = frame.source.input(message_index) else {
        return;
    };
    if line.separator {
        if !line.text.is_empty() {
            let active = heading_is_active(frame, message_index, width);
            draw_section_header(buf, line, y, frame.content_x, width, active);
        }
        return;
    }
    let first = row == frame.cache.first_content_row(message_index);
    let active = frame.active == Some(message_index);
    fill_message_row(buf, y, line.role);
    if !frame.flow {
        draw_gutter(buf, message, y, first, active);
    }
    let selection = frame.viewport.selected_cells(index, line.cells);
    draw_rendered_line(
        buf,
        frame.content_x,
        y,
        width,
        line,
        role_text_style(line.role),
        selection,
    );
    if active {
        let last = frame
            .cache
            .block_len(message_index)
            .is_some_and(|length| row + 1 == length);
        let inline = frame.flow && inline_reasoning(message, width);
        paint_active_step(
            buf,
            y,
            width,
            &line.text,
            selection,
            (last, inline),
            frame.content_x,
        );
    }
}

fn heading_is_active<S: TranscriptSource + ?Sized>(
    frame: &TranscriptFrame<'_, S>,
    index: usize,
    width: usize,
) -> bool {
    frame.active.is_some_and(|active| {
        index <= active
            && (index..=active).all(|row| {
                frame.source.input(row).is_some_and(|message| {
                    message.role == Role::Reasoning && !inline_reasoning(message, width)
                })
            })
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
    line: &super::view_models::RenderedLine,
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

fn draw_gutter(buf: &mut Buffer, message: RenderInput<'_>, y: usize, first: bool, active: bool) {
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
