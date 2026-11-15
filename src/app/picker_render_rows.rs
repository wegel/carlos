//! Paint one borrowed saved-session row without copying native metadata.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::picker_render_time::format_picker_timestamp;
use super::{PickerColumns, PickerLayout, PickerRow};
use crate::app::render::{draw_str, fill_rect};
use crate::app::text::{split_at_cells, visual_width};
use crate::theme::{COLOR_DIM, COLOR_PRIMARY, COLOR_STEP1, COLOR_TEXT};

// --- Row Rendering ---

pub(super) fn draw_picker_list_row(
    buf: &mut Buffer,
    layout: &PickerLayout,
    cols: &PickerColumns,
    t: &PickerRow<'_>,
    active: bool,
    row: usize,
) {
    let y = layout.list_y + 1 + row;
    if active && layout.list_w > 0 {
        fill_rect(
            buf,
            layout.list_x,
            y,
            layout.list_w,
            1,
            Style::default().bg(COLOR_PRIMARY),
        );
    }
    let (bullet_style, line_style) = row_styles(active);
    draw_str(
        buf,
        layout.list_x,
        y,
        if active { "●" } else { " " },
        bullet_style,
        1,
    );
    draw_str(
        buf,
        layout.list_x + 2,
        y,
        &format_row_left_text(t, cols.left_col_w),
        line_style,
        cols.left_col_w.saturating_sub(2),
    );
    if cols.ts_col_w > 0 {
        draw_row_timestamps(buf, layout, cols, t, y, line_style);
    }
}

fn draw_row_timestamps(
    buf: &mut Buffer,
    layout: &PickerLayout,
    cols: &PickerColumns,
    t: &PickerRow<'_>,
    y: usize,
    line_style: Style,
) {
    draw_str(
        buf,
        layout.list_x + cols.left_col_w + cols.gap_w,
        y,
        &format_picker_timestamp(t.created_at),
        line_style,
        cols.ts_col_w,
    );
    draw_str(
        buf,
        layout.list_x + cols.left_col_w + cols.gap_w + cols.ts_col_w + cols.gap_w,
        y,
        &format_picker_timestamp(t.updated_at),
        line_style,
        cols.ts_col_w,
    );
}

fn format_row_left_text(t: &PickerRow<'_>, left_col_w: usize) -> String {
    let preview_w = if left_col_w > 32 { left_col_w - 32 } else { 10 };
    let label = t.name.unwrap_or(t.preview);
    let label = if visual_width(label) > preview_w {
        let cut = split_at_cells(label, preview_w);
        &label[..cut]
    } else {
        label
    };

    let cwd_tail = if t.cwd.is_empty() { "" } else { t.cwd };
    let mut left = format!("{}  {}  {}", t.id, label, cwd_tail);
    let left_view_w = left_col_w.saturating_sub(2);
    if visual_width(&left) > left_view_w {
        let cut = split_at_cells(&left, left_view_w);
        left.truncate(cut);
    }
    left
}

fn row_styles(active: bool) -> (Style, Style) {
    if active {
        (
            Style::default().fg(COLOR_STEP1).bg(COLOR_PRIMARY),
            Style::default()
                .fg(COLOR_STEP1)
                .bg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        (
            Style::default().fg(COLOR_DIM),
            Style::default().fg(COLOR_TEXT),
        )
    }
}
