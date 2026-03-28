use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::render::{draw_str, fill_rect};
use super::text::{split_at_cells, visual_width};
use super::{TerminalSize, ThreadSummary};
use crate::theme::{
    COLOR_DIFF_REMOVE, COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_STEP1, COLOR_STEP2,
    COLOR_STEP7, COLOR_TEXT,
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

pub(super) fn draw_picker(
    frame: &mut ratatui::Frame<'_>,
    threads: &[ThreadSummary],
    selected: usize,
    top: usize,
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

    for y in layout.panel_y..(layout.panel_y + layout.panel_h) {
        draw_str(
            buf,
            layout.panel_x,
            y,
            "┃",
            Style::default().fg(COLOR_STEP7),
            1,
        );
        if layout.panel_w > 1 {
            draw_str(
                buf,
                layout.panel_x + layout.panel_w - 1,
                y,
                "┃",
                Style::default().fg(COLOR_STEP7),
                1,
            );
        }
    }

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
        "Enter open  j/k move  g/G jump  d delete",
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );

    let ts_col_w = 16usize.min(layout.list_w.saturating_sub(8));
    let gap_w: usize = if layout.list_w > ts_col_w { 2 } else { 0 };
    let left_col_w = layout
        .list_w
        .saturating_sub(ts_col_w.saturating_mul(2))
        .saturating_sub(gap_w.saturating_mul(2));
    let data_rows = layout.list_h.saturating_sub(1);

    if layout.list_h > 0 {
        draw_str(
            buf,
            layout.list_x + 2,
            layout.list_y,
            "Session",
            Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
            left_col_w.saturating_sub(2),
        );
        if ts_col_w > 0 {
            draw_str(
                buf,
                layout.list_x + left_col_w + gap_w,
                layout.list_y,
                "Created",
                Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                ts_col_w,
            );
            draw_str(
                buf,
                layout.list_x + left_col_w + gap_w + ts_col_w + gap_w,
                layout.list_y,
                "Last Updated",
                Style::default().fg(COLOR_DIM).add_modifier(Modifier::BOLD),
                ts_col_w,
            );
        }
    }

    for row in 0..data_rows {
        let idx = top + row;
        let y = layout.list_y + 1 + row;
        if idx >= threads.len() {
            continue;
        }

        let t = &threads[idx];
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
        let created = format_picker_timestamp(t.created_at);
        let updated = format_picker_timestamp(t.updated_at);

        let active = idx == selected;
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

        let bullet_style = if active {
            Style::default().fg(COLOR_STEP1).bg(COLOR_PRIMARY)
        } else {
            Style::default().fg(COLOR_DIM)
        };
        let line_style = if active {
            Style::default()
                .fg(COLOR_STEP1)
                .bg(COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(COLOR_TEXT)
        };

        draw_str(
            buf,
            layout.list_x,
            y,
            if active { "●" } else { " " },
            bullet_style,
            1,
        );
        draw_str(buf, layout.list_x + 2, y, &left, line_style, left_view_w);
        if ts_col_w > 0 {
            draw_str(
                buf,
                layout.list_x + left_col_w + gap_w,
                y,
                &created,
                line_style,
                ts_col_w,
            );
            draw_str(
                buf,
                layout.list_x + left_col_w + gap_w + ts_col_w + gap_w,
                y,
                &updated,
                line_style,
                ts_col_w,
            );
        }
    }

    let footer = status
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("{} sessions", threads.len()));
    draw_str(
        buf,
        layout.list_x,
        layout.panel_y + layout.panel_h - 2,
        &footer,
        Style::default().fg(COLOR_DIM),
        layout.list_w,
    );

    if let Some(target) = delete_target {
        draw_picker_delete_dialog(buf, size, target);
    }
}

pub(super) fn draw_picker_delete_dialog(
    buf: &mut Buffer,
    size: TerminalSize,
    target: &ThreadSummary,
) {
    if size.width < 24 || size.height < 8 {
        return;
    }

    let dialog_w = size.width.saturating_sub(8).min(72).max(24);
    let dialog_h = 8usize;
    let left = (size.width.saturating_sub(dialog_w)) / 2;
    let top = (size.height.saturating_sub(dialog_h)) / 2;
    let right = left + dialog_w.saturating_sub(1);
    let bottom = top + dialog_h.saturating_sub(1);
    let text_w = dialog_w.saturating_sub(4);

    fill_rect(
        buf,
        left,
        top,
        dialog_w,
        dialog_h,
        Style::default().bg(COLOR_STEP2),
    );
    draw_str(
        buf,
        left,
        top,
        "┏",
        Style::default().fg(COLOR_DIFF_REMOVE),
        1,
    );
    draw_str(
        buf,
        right,
        top,
        "┓",
        Style::default().fg(COLOR_DIFF_REMOVE),
        1,
    );
    draw_str(
        buf,
        left,
        bottom,
        "┗",
        Style::default().fg(COLOR_DIFF_REMOVE),
        1,
    );
    draw_str(
        buf,
        right,
        bottom,
        "┛",
        Style::default().fg(COLOR_DIFF_REMOVE),
        1,
    );
    for x in (left + 1)..right {
        draw_str(buf, x, top, "─", Style::default().fg(COLOR_DIFF_REMOVE), 1);
        draw_str(
            buf,
            x,
            bottom,
            "─",
            Style::default().fg(COLOR_DIFF_REMOVE),
            1,
        );
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, "┃", Style::default().fg(COLOR_DIFF_REMOVE), 1);
        draw_str(
            buf,
            right,
            y,
            "┃",
            Style::default().fg(COLOR_DIFF_REMOVE),
            1,
        );
    }

    draw_str(
        buf,
        left + 2,
        top + 1,
        "Delete session?",
        Style::default().fg(COLOR_TEXT).add_modifier(Modifier::BOLD),
        text_w,
    );
    draw_str(
        buf,
        left + 2,
        top + 3,
        target.name.as_deref().unwrap_or(&target.preview),
        Style::default().fg(COLOR_TEXT),
        text_w,
    );
    draw_str(
        buf,
        left + 2,
        top + 4,
        &target.id,
        Style::default().fg(COLOR_DIM),
        text_w,
    );
    draw_str(
        buf,
        left + 2,
        top + 6,
        "y/Enter delete  n/Esc cancel",
        Style::default().fg(COLOR_DIM),
        text_w,
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
