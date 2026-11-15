//! Main rendering pipeline: transcript drawing, input area, and status bar.

#[path = "render_sections.rs"]
mod render_sections;

use ratatui::buffer::Buffer;
use ratatui::style::Style;

use super::text::{slice_by_cells, visual_width};

use super::RenderedLine;

use crate::theme::{COLOR_DIFF_REMOVE, COLOR_STATUS_SEPARATOR, COLOR_STEP8, COLOR_TEXT};

pub(super) use self::render_sections::draw_composer;

// --- Re-exports: items moved to render_input but accessed via super::render:: by other modules ---

pub(super) use super::render_input::textarea_input_from_key;

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

/// Draw the same separator beneath a native-backed or client-backed transcript.
pub(super) fn draw_status_separator(buf: &mut Buffer, y: usize, width: usize, rewind: bool) {
    let color = if rewind {
        COLOR_DIFF_REMOVE
    } else {
        COLOR_STATUS_SEPARATOR
    };
    draw_str(
        buf,
        0,
        y,
        &"━".repeat(width),
        Style::default().fg(color),
        width,
    );
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

struct SegmentRow {
    y: usize,
    x_origin: usize,
    max_width: usize,
    base_style: Style,
    selection: Option<(usize, usize)>,
}

struct SegmentCursor {
    draw_x: usize,
    col: usize,
}

fn render_segment(
    buf: &mut Buffer,
    row: &SegmentRow,
    text: &str,
    seg_style: Style,
    cursor: &mut SegmentCursor,
) {
    let SegmentRow {
        y,
        x_origin,
        max_width,
        base_style,
        selection,
    } = *row;
    let SegmentCursor { draw_x, col } = cursor;
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
    let row = SegmentRow {
        y,
        x_origin: x,
        max_width,
        base_style,
        selection,
    };
    let mut cursor = SegmentCursor { draw_x: x, col: 0 };
    if line.styled_segments.is_empty() {
        render_segment(buf, &row, &line.text, Style::default(), &mut cursor);
        return;
    }
    for seg in &line.styled_segments {
        if cursor.draw_x >= x + max_width {
            break;
        }
        render_segment(buf, &row, &seg.text, seg.style, &mut cursor);
    }
    if cursor.col < line.cells && cursor.draw_x < x + max_width {
        let tail = slice_by_cells(&line.text, cursor.col, line.cells);
        render_segment(buf, &row, &tail, Style::default(), &mut cursor);
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

// --- Transcript viewport ---
