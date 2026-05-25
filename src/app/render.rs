//! Main rendering pipeline: transcript drawing, input area, and status bar.

#[path = "render_sections.rs"]
mod render_sections;

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::selection::compute_selection_range;
use super::text::{slice_by_cells, visual_width};
use super::transcript_render::transcript_content_width;
use super::{AppState, RenderedLine, Role, TerminalSize, MSG_CONTENT_X, MSG_TOP};
use crate::theme::{
    role_fg, role_gutter_fg, role_gutter_symbol, role_row_bg, COLOR_STEP1, COLOR_STEP2,
    COLOR_STEP6, COLOR_STEP8, COLOR_TEXT,
};

use self::render_sections::{cursor_position, draw_input_area, draw_overlays, draw_status_bar};

// --- Re-exports: items moved to render_input but accessed via super::render:: by other modules ---

pub(super) use super::render_input::{
    compute_input_layout, is_newline_enter, normalize_pasted_text, textarea_input_from_key,
};

// --- Drawing primitives (used by picker_render, overlay_render, and render_lines) ---

pub(super) fn draw_str(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    text: &str,
    style: Style,
    max_width: usize,
) {
    if text.is_empty() || max_width == 0 {
        return;
    }
    if let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) {
        buf.set_stringn(x, y, text, max_width, style);
    }
}

fn draw_piece(
    buf: &mut Buffer,
    y: usize,
    x_origin: usize,
    max_width: usize,
    piece: &str,
    piece_style: Style,
    draw_x: &mut usize,
) {
    if piece.is_empty() || *draw_x >= x_origin + max_width {
        return;
    }
    let rem = max_width.saturating_sub(*draw_x - x_origin);
    draw_str(buf, *draw_x, y, piece, piece_style, rem);
    *draw_x += visual_width(piece);
}

fn render_segment(
    buf: &mut Buffer,
    y: usize,
    x_origin: usize,
    max_width: usize,
    text: &str,
    base_style: Style,
    seg_style: Style,
    selection: Option<(usize, usize)>,
    draw_x: &mut usize,
    col: &mut usize,
) {
    if *draw_x >= x_origin + max_width || text.is_empty() {
        return;
    }
    let seg_cells = visual_width(text);
    if seg_cells == 0 {
        return;
    }
    let style = base_style.patch(seg_style);
    let seg_start = *col;
    let seg_end = seg_start + seg_cells;
    if let Some((sel_start, sel_end)) = selection {
        if sel_end <= seg_start || sel_start >= seg_end {
            draw_piece(buf, y, x_origin, max_width, text, style, draw_x);
        } else {
            let local_start = sel_start.saturating_sub(seg_start).min(seg_cells);
            let local_end = sel_end.saturating_sub(seg_start).min(seg_cells);
            let before = slice_by_cells(text, 0, local_start);
            let selected = slice_by_cells(text, local_start, local_end);
            let after = slice_by_cells(text, local_end, seg_cells);
            draw_piece(buf, y, x_origin, max_width, &before, style, draw_x);
            draw_piece(
                buf,
                y,
                x_origin,
                max_width,
                &selected,
                style.fg(COLOR_TEXT).bg(COLOR_STEP8),
                draw_x,
            );
            draw_piece(buf, y, x_origin, max_width, &after, style, draw_x);
        }
    } else {
        draw_piece(buf, y, x_origin, max_width, text, style, draw_x);
    }
    *col = seg_end;
}

pub(super) fn draw_rendered_line(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    max_width: usize,
    line: &RenderedLine,
    base_style: Style,
    selection: Option<(usize, usize)>,
) {
    if max_width == 0 || line.cells == 0 {
        return;
    }
    if !line.styled_segments.is_empty() {
        draw_str(buf, x, y, &line.text, base_style, max_width);
    }
    let mut draw_x = x;
    let mut col = 0usize;
    let mut emit = |text: &str, style: Style, draw_x: &mut usize, col: &mut usize| {
        render_segment(
            buf, y, x, max_width, text, base_style, style, selection, draw_x, col,
        );
    };

    if line.styled_segments.is_empty() {
        emit(&line.text, Style::default(), &mut draw_x, &mut col);
        return;
    }
    for seg in &line.styled_segments {
        if draw_x >= x + max_width {
            break;
        }
        emit(&seg.text, seg.style, &mut draw_x, &mut col);
    }
    if col < line.cells && draw_x < x + max_width {
        let tail = slice_by_cells(&line.text, col, line.cells);
        emit(&tail, Style::default(), &mut draw_x, &mut col);
    }
}

pub(super) fn fill_rect(buf: &mut Buffer, x: usize, y: usize, w: usize, h: usize, style: Style) {
    if w == 0 || h == 0 {
        return;
    }
    if let (Ok(x), Ok(y), Ok(w), Ok(h)) = (
        u16::try_from(x),
        u16::try_from(y),
        u16::try_from(w),
        u16::try_from(h),
    ) {
        let blank = " ".repeat(w as usize);
        for row in 0..h {
            buf.set_stringn(x, y + row, &blank, w as usize, style);
        }
    }
}

// --- Main view rendering ---

pub(super) fn render_main_view(frame: &mut ratatui::Frame<'_>, app: &mut AppState) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };
    if size.width == 0 || size.height == 0 {
        return;
    }

    // --- Layout ---
    let input_layout_started = Instant::now();
    let input_layout = compute_input_layout(app, size);
    if let Some(perf) = app.perf.as_mut() {
        perf.input_layout.push(input_layout_started.elapsed());
    }
    let msg_height = input_layout
        .msg_bottom
        .checked_sub(MSG_TOP)
        .map_or(0, |rows| rows + 1);
    let msg_width = transcript_content_width(size);
    sync_transcript_viewport(app, msg_height);

    // --- Draw ---
    let buf = frame.buffer_mut();
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    if msg_height > 0 {
        fill_rect(
            buf,
            0,
            MSG_TOP - 1,
            size.width,
            msg_height,
            Style::default().bg(COLOR_STEP2),
        );
    }
    draw_transcript(buf, app, msg_height, msg_width);
    draw_input_area(buf, app, size, &input_layout);
    draw_status_bar(buf, app, size, input_layout.input_top);
    draw_overlays(buf, size, app);

    // --- Cursor ---
    let (cursor_x, cursor_y) = cursor_position(app, size, &input_layout);
    frame.set_cursor_position((cursor_x as u16, cursor_y as u16));
}

// --- Transcript viewport ---

fn sync_transcript_viewport(app: &mut AppState, msg_height: usize) {
    let total_lines = app.rendered_line_count();
    let max_scroll = total_lines.saturating_sub(msg_height);
    if app.viewport.scroll_top > max_scroll {
        app.viewport.scroll_top = max_scroll;
    }
    if app.viewport.auto_follow_bottom && max_scroll > 0 {
        app.viewport.scroll_top = max_scroll;
    }
    if msg_height == 0 || total_lines == 0 {
        return;
    }
    let end_idx = (app.viewport.scroll_top + msg_height)
        .saturating_sub(1)
        .min(total_lines - 1);
    app.ensure_rendered_range_materialized(app.viewport.scroll_top, end_idx);
}

// --- Transcript ---

fn draw_transcript(buf: &mut Buffer, app: &AppState, msg_height: usize, msg_width: usize) {
    for row in 0..msg_height {
        let line_idx = app.viewport.scroll_top + row;
        let y = MSG_TOP + row - 1;
        draw_transcript_row(buf, app, line_idx, y, msg_width);
    }
}

fn draw_transcript_row(
    buf: &mut Buffer,
    app: &AppState,
    line_idx: usize,
    y: usize,
    msg_width: usize,
) {
    let line_opt = app.rendered_line_at(line_idx);
    if let Some(line) = line_opt.filter(|line| !line.separator) {
        draw_role_gutter(buf, y, line.role);
    }
    if msg_width == 0 {
        return;
    }
    let Some(line) = line_opt else {
        return;
    };
    fill_rect(
        buf,
        MSG_CONTENT_X,
        y,
        msg_width,
        1,
        Style::default().bg(role_row_bg(line.role)),
    );
    if line.separator {
        draw_separator_row(buf, y, msg_width);
        return;
    }
    let selection = compute_row_selection(app, line_idx, line.cells);
    draw_rendered_line(
        buf,
        MSG_CONTENT_X,
        y,
        msg_width,
        line,
        transcript_line_style(line.role),
        selection,
    );
}

fn draw_role_gutter(buf: &mut Buffer, y: usize, role: Role) {
    draw_str(
        buf,
        0,
        y,
        role_gutter_symbol(role),
        Style::default()
            .fg(role_gutter_fg(role))
            .add_modifier(Modifier::BOLD),
        1,
    );
}

fn draw_separator_row(buf: &mut Buffer, y: usize, msg_width: usize) {
    let sep = "─".repeat(msg_width);
    draw_str(
        buf,
        MSG_CONTENT_X,
        y,
        &sep,
        Style::default().fg(COLOR_STEP6),
        msg_width,
    );
}

fn compute_row_selection(
    app: &AppState,
    line_idx: usize,
    line_cells: usize,
) -> Option<(usize, usize)> {
    app.viewport
        .selection
        .and_then(|sel| compute_selection_range(sel, line_idx, line_cells))
        .map(|(start, end)| (start.min(line_cells), end.min(line_cells)))
}

fn transcript_line_style(role: Role) -> Style {
    let style = Style::default().fg(role_fg(role));
    if matches!(role, Role::Reasoning) {
        style.add_modifier(Modifier::DIM)
    } else if matches!(role, Role::Commentary) {
        style.add_modifier(Modifier::DIM | Modifier::ITALIC)
    } else {
        style
    }
}
