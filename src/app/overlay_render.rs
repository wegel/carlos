use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::perf::PerfMetrics;
use super::render::{draw_str, fill_rect};
use super::state::ModelSettingsField;
use super::text::{visual_width, wrap_natural_by_cells};
use super::{AppState, TerminalSize};
use crate::theme::{
    COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_STEP1, COLOR_STEP2, COLOR_STEP6, COLOR_STEP7,
    COLOR_TEXT,
};

fn draw_box_border(
    buf: &mut Buffer,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
    style: Style,
    corners: (&str, &str, &str, &str),
    horizontal: &str,
    vertical: &str,
) {
    draw_str(buf, left, top, corners.0, style, 1);
    draw_str(buf, right, top, corners.1, style, 1);
    draw_str(buf, left, bottom, corners.2, style, 1);
    draw_str(buf, right, bottom, corners.3, style, 1);
    for x in (left + 1)..right {
        draw_str(buf, x, top, horizontal, style, 1);
        draw_str(buf, x, bottom, horizontal, style, 1);
    }
    for y in (top + 1)..bottom {
        draw_str(buf, left, y, vertical, style, 1);
        draw_str(buf, right, y, vertical, style, 1);
    }
}

pub(super) fn draw_help_overlay(buf: &mut Buffer, size: TerminalSize) {
    if !(size.height > 10 && size.width > 44) {
        return;
    }

    let box_w = (size.width - 8).min(74);
    let box_h = 10usize;
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

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
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;
    draw_box_border(
        buf,
        left,
        top,
        right,
        bottom,
        Style::default().fg(COLOR_STEP7),
        ("┏", "┓", "┗", "┛"),
        "─",
        "┃",
    );

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        "Help",
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + box_w.saturating_sub(8),
        start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 3,
        "Enter send/steer  Shift/Alt+Enter newline",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 4,
        "Ctrl+Y copy selection or last answer",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 5,
        "Home/End jump transcript  Ctrl+M model settings",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 6,
        "Wheel scroll, drag to select, release to copy",
        Style::default().fg(COLOR_TEXT),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 7,
        "? toggle this help",
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

pub(super) fn draw_model_settings_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    if !(size.height > 14 && size.width > 56) {
        return;
    }

    let box_w = (size.width - 10).min(80);
    let box_h = 12usize;
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;
    draw_box_border(
        buf,
        left,
        top,
        right,
        bottom,
        Style::default().fg(COLOR_STEP7),
        ("┏", "┓", "┗", "┛"),
        "─",
        "┃",
    );

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        if app.runtime_supports_summary() {
            "Model / Thinking / Summary"
        } else {
            "Model / Thinking"
        },
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + box_w.saturating_sub(8),
        start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );

    let model_style = if matches!(app.runtime.model_settings_field, ModelSettingsField::Model) {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    };
    let effort_style = if matches!(app.runtime.model_settings_field, ModelSettingsField::Effort) {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    };
    let summary_style = if matches!(app.runtime.model_settings_field, ModelSettingsField::Summary) {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    };

    draw_str(
        buf,
        start_x + 3,
        start_y + 3,
        "Model",
        Style::default().fg(COLOR_DIM),
        8,
    );
    draw_str(
        buf,
        start_x + 12,
        start_y + 3,
        app.model_settings_model_value(),
        model_style,
        box_w.saturating_sub(16),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 5,
        "Thinking",
        Style::default().fg(COLOR_DIM),
        8,
    );
    draw_str(
        buf,
        start_x + 12,
        start_y + 5,
        app.model_settings_effort_value(),
        effort_style,
        box_w.saturating_sub(16),
    );
    if app.runtime_supports_summary() {
        draw_str(
            buf,
            start_x + 3,
            start_y + 7,
            "Summary",
            Style::default().fg(COLOR_DIM),
            8,
        );
        draw_str(
            buf,
            start_x + 12,
            start_y + 7,
            app.model_settings_summary_value(),
            summary_style,
            box_w.saturating_sub(16),
        );
    }
    draw_str(
        buf,
        start_x + 3,
        if app.runtime_supports_summary() {
            start_y + 9
        } else {
            start_y + 7
        },
        "Tab switch field, arrows adjust, Enter apply",
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

pub(super) fn draw_approval_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    let Some(approval) = app.approval.pending.as_ref() else {
        return;
    };
    if size.width < 36 || size.height < 10 {
        return;
    }

    let inner_w = size.width.saturating_sub(12).min(92);
    let mut detail_lines = Vec::new();
    for line in &approval.detail_lines {
        let wrapped = wrap_natural_by_cells(line, inner_w.saturating_sub(6).max(8));
        if wrapped.is_empty() {
            detail_lines.push(String::new());
        } else {
            detail_lines.extend(wrapped);
        }
    }
    if detail_lines.is_empty() {
        detail_lines.push("No additional detail provided.".to_string());
    }

    let mut footer = vec!["y accept".to_string(), "n decline".to_string()];
    if approval.can_accept_for_session {
        footer.push("s accept session".to_string());
    }
    if approval.can_cancel {
        footer.push("c cancel turn".to_string());
    }
    let footer_text = footer.join("  ");

    let max_body_lines = size.height.saturating_sub(7);
    if detail_lines.len() > max_body_lines {
        detail_lines.truncate(max_body_lines);
        if let Some(last) = detail_lines.last_mut() {
            *last = "…".to_string();
        }
    }

    let box_w = inner_w;
    let box_h = (detail_lines.len() + 5).min(size.height.saturating_sub(2));
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;

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
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;
    draw_box_border(
        buf,
        left,
        top,
        right,
        bottom,
        Style::default().fg(COLOR_STEP7),
        ("┏", "┓", "┗", "┛"),
        "─",
        "┃",
    );

    draw_str(
        buf,
        start_x + 3,
        start_y + 1,
        &approval.title,
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        start_x + 3,
        start_y + 2,
        &approval.method,
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );

    for (i, line) in detail_lines.iter().enumerate() {
        draw_str(
            buf,
            start_x + 3,
            start_y + 3 + i,
            line,
            Style::default().fg(COLOR_TEXT),
            box_w.saturating_sub(6),
        );
    }

    draw_str(
        buf,
        start_x + 3,
        bottom.saturating_sub(1),
        &footer_text,
        Style::default().fg(COLOR_DIM),
        box_w.saturating_sub(6),
    );
}

pub(super) fn draw_perf_overlay(buf: &mut Buffer, size: TerminalSize, perf: &PerfMetrics) {
    let lines = perf.overlay_lines();
    if lines.is_empty() || size.width < 44 || size.height < lines.len() + 4 {
        return;
    }

    let inner_w = lines
        .iter()
        .map(|line| visual_width(line))
        .max()
        .unwrap_or(0)
        .min(size.width.saturating_sub(6));
    if inner_w == 0 {
        return;
    }

    let box_w = inner_w + 4;
    let box_h = lines.len() + 2;
    let start_x = size.width.saturating_sub(box_w + 2);
    let start_y = 1usize;
    if start_y + box_h > size.height {
        return;
    }

    fill_rect(
        buf,
        start_x,
        start_y,
        box_w,
        box_h,
        Style::default().bg(COLOR_STEP1),
    );

    let left = start_x;
    let right = start_x + box_w - 1;
    let top = start_y;
    let bottom = start_y + box_h - 1;
    draw_box_border(
        buf,
        left,
        top,
        right,
        bottom,
        Style::default().fg(COLOR_STEP7),
        ("┌", "┐", "└", "┘"),
        "─",
        "│",
    );

    for (i, line) in lines.iter().enumerate() {
        draw_str(
            buf,
            start_x + 2,
            start_y + 1 + i,
            line,
            Style::default().fg(COLOR_DIM),
            inner_w,
        );
    }
}
