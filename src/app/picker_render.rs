use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::picker_delete_dialog::draw_picker_delete_dialog;
use super::render::{draw_str, fill_rect};
use super::text::{split_at_cells, visual_width};
use super::{TerminalSize, ThreadSummary};
use crate::theme::{
    COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_STEP1, COLOR_STEP2, COLOR_STEP7, COLOR_TEXT,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct PickerLayout {
    pub(super) panel_x: usize,
    pub(super) panel_y: usize,
    pub(super) panel_w: usize,
    pub(super) panel_h: usize,
    pub(super) list_x: usize,
    pub(super) list_y: usize,
    pub(super) list_w: usize,
    pub(super) list_h: usize,
}

pub(super) fn compute_picker_layout(size: TerminalSize) -> PickerLayout {
    let panel_w = if size.width > 6 {
        size.width - 2
    } else {
        size.width
    };
    let panel_h = if size.height > 4 {
        size.height - 2
    } else {
        size.height
    };
    let panel_x = if size.width > panel_w {
        (size.width - panel_w) / 2
    } else {
        0
    };
    let panel_y = if size.height > panel_h {
        (size.height - panel_h) / 3
    } else {
        0
    };

    let list_x = panel_x + 2;
    let list_y = panel_y + 3;
    let list_w = panel_w.saturating_sub(4);
    let list_h = if panel_h > 6 { panel_h - 6 } else { 1 };

    PickerLayout {
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        list_x,
        list_y,
        list_w,
        list_h,
    }
}

/// Column widths for the picker list.
struct PickerColumns {
    left_col_w: usize,
    ts_col_w: usize,
    gap_w: usize,
    data_rows: usize,
}

fn compute_picker_columns(layout: &PickerLayout) -> PickerColumns {
    let ts_col_w = 16usize.min(layout.list_w.saturating_sub(8));
    let gap_w: usize = if layout.list_w > ts_col_w { 2 } else { 0 };
    let left_col_w = layout
        .list_w
        .saturating_sub(ts_col_w.saturating_mul(2))
        .saturating_sub(gap_w.saturating_mul(2));
    let data_rows = layout.list_h.saturating_sub(1);
    PickerColumns {
        left_col_w,
        ts_col_w,
        gap_w,
        data_rows,
    }
}

pub(super) fn draw_picker(
    frame: &mut ratatui::Frame<'_>,
    threads: &[ThreadSummary],
    selected: usize,
    top: usize,
    allow_delete: bool,
    delete_target: Option<&ThreadSummary>,
    status: Option<&str>,
) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let layout = compute_picker_layout(size);
    let buf = frame.buffer_mut();

    draw_picker_background(buf, size, &layout);

    if layout.panel_w < 8 || layout.panel_h < 7 || layout.list_w == 0 {
        draw_str(
            buf,
            1,
            0,
            "carlos resume",
            Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
            size.width.saturating_sub(1),
        );
        return;
    }

    draw_picker_panel_borders(buf, &layout);
    draw_picker_header(buf, &layout, allow_delete);

    let cols = compute_picker_columns(&layout);
    draw_picker_column_headers(buf, &layout, &cols);

    for row in 0..cols.data_rows {
        let idx = top + row;
        if idx < threads.len() {
            draw_picker_list_row(buf, &layout, &cols, &threads[idx], idx == selected, row);
        }
    }

    draw_picker_footer(buf, &layout, threads.len(), status);

    if let Some(target) = delete_target {
        draw_picker_delete_dialog(buf, size, target);
    }
}

fn draw_picker_background(buf: &mut Buffer, size: TerminalSize, layout: &PickerLayout) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
    fill_rect(
        buf,
        layout.panel_x,
        layout.panel_y,
        layout.panel_w,
        layout.panel_h,
        Style::default().bg(COLOR_STEP2),
    );
}

fn draw_picker_panel_borders(buf: &mut Buffer, layout: &PickerLayout) {
    let border_style = Style::default().fg(COLOR_STEP7);
    for y in layout.panel_y..(layout.panel_y + layout.panel_h) {
        draw_str(buf, layout.panel_x, y, "┃", border_style, 1);
        if layout.panel_w > 1 {
            draw_str(
                buf,
                layout.panel_x + layout.panel_w - 1,
                y,
                "┃",
                border_style,
                1,
            );
        }
    }
}

fn draw_picker_header(buf: &mut Buffer, layout: &PickerLayout, allow_delete: bool) {
    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + 1,
        "Sessions",
        Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        layout.list_w,
    );
    if layout.panel_w > 5 {
        draw_str(
            buf,
            layout.panel_x + layout.panel_w - 5,
            layout.panel_y + 1,
            "esc",
            Style::default().fg(COLOR_DIM),
            3,
        );
    }
    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + 2,
        if allow_delete {
            "Enter open  j/k move  g/G jump  d delete"
        } else {
            "Enter open  j/k move  g/G jump"
        },
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );
}

fn draw_picker_column_headers(
    buf: &mut Buffer,
    layout: &PickerLayout,
    cols: &PickerColumns,
) {
    if layout.list_h == 0 {
        return;
    }
    let header_style = Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD);
    draw_str(
        buf,
        layout.list_x + 2,
        layout.list_y,
        "Session",
        header_style,
        cols.left_col_w.saturating_sub(2),
    );
    if cols.ts_col_w > 0 {
        draw_str(
            buf,
            layout.list_x + cols.left_col_w + cols.gap_w,
            layout.list_y,
            "Created",
            header_style,
            cols.ts_col_w,
        );
        draw_str(
            buf,
            layout.list_x + cols.left_col_w + cols.gap_w + cols.ts_col_w + cols.gap_w,
            layout.list_y,
            "Last Updated",
            header_style,
            cols.ts_col_w,
        );
    }
}

fn draw_picker_list_row(
    buf: &mut Buffer,
    layout: &PickerLayout,
    cols: &PickerColumns,
    t: &ThreadSummary,
    active: bool,
    row: usize,
) {
    let y = layout.list_y + 1 + row;
    let left_text = format_row_left_text(t, cols.left_col_w);
    let created = format_picker_timestamp(t.created_at);
    let updated = format_picker_timestamp(t.updated_at);

    if active && layout.list_w > 0 {
        fill_rect(buf, layout.list_x, y, layout.list_w, 1, Style::default().bg(COLOR_PRIMARY));
    }

    let (bullet_style, line_style) = row_styles(active);
    let left_view_w = cols.left_col_w.saturating_sub(2);

    draw_str(buf, layout.list_x, y, if active { "●" } else { " " }, bullet_style, 1);
    draw_str(buf, layout.list_x + 2, y, &left_text, line_style, left_view_w);
    if cols.ts_col_w > 0 {
        draw_str(buf, layout.list_x + cols.left_col_w + cols.gap_w, y, &created, line_style, cols.ts_col_w);
        draw_str(
            buf,
            layout.list_x + cols.left_col_w + cols.gap_w + cols.ts_col_w + cols.gap_w,
            y,
            &updated,
            line_style,
            cols.ts_col_w,
        );
    }
}

fn format_row_left_text(t: &ThreadSummary, left_col_w: usize) -> String {
    let preview_w = if left_col_w > 32 { left_col_w - 32 } else { 10 };
    let label = t.name.as_deref().unwrap_or(&t.preview);
    let label = if visual_width(label) > preview_w {
        let cut = split_at_cells(label, preview_w);
        &label[..cut]
    } else {
        label
    };

    let cwd_tail = if t.cwd.is_empty() { "" } else { &t.cwd };
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

fn draw_picker_footer(
    buf: &mut Buffer,
    layout: &PickerLayout,
    thread_count: usize,
    status: Option<&str>,
) {
    let footer = status
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{} sessions", thread_count));
    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + layout.panel_h - 2,
        &footer,
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );
}

fn format_picker_timestamp(ts: i64) -> String {
    if ts <= 0 {
        return "-".to_string();
    }

    let days = ts.div_euclid(86_400);
    let secs_of_day = ts.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn civil_from_days(days_since_unix_epoch: i64) -> (i64, i64, i64) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    if m <= 2 {
        y += 1;
    }
    (y, m, d)
}
