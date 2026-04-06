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

struct OverlayBox {
    start_x: usize,
    start_y: usize,
    box_w: usize,
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

fn compute_overlay_box(size: TerminalSize, box_w: usize, box_h: usize) -> OverlayBox {
    let start_x = (size.width - box_w) / 2;
    let start_y = (size.height - box_h) / 2;
    OverlayBox {
        start_x,
        start_y,
        box_w,
        left: start_x,
        right: start_x + box_w - 1,
        top: start_y,
        bottom: start_y + box_h - 1,
    }
}

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

fn draw_rounded_border(buf: &mut Buffer, ob: &OverlayBox) {
    draw_box_border(
        buf,
        ob.left,
        ob.top,
        ob.right,
        ob.bottom,
        Style::default().fg(COLOR_STEP7),
        ("┏", "┓", "┗", "┛"),
        "─",
        "┃",
    );
}

fn draw_overlay_title(buf: &mut Buffer, ob: &OverlayBox, title: &str) {
    draw_str(
        buf,
        ob.start_x + 3,
        ob.start_y + 1,
        title,
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        ob.box_w.saturating_sub(6),
    );
    draw_str(
        buf,
        ob.start_x + ob.box_w.saturating_sub(8),
        ob.start_y + 1,
        "esc",
        Style::default().fg(COLOR_DIM),
        3,
    );
}

fn fill_fullscreen_overlay(buf: &mut Buffer, size: TerminalSize) {
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_OVERLAY),
    );
}

fn fill_box_background(buf: &mut Buffer, ob: &OverlayBox, box_h: usize) {
    fill_rect(
        buf,
        ob.start_x,
        ob.start_y,
        ob.box_w,
        box_h,
        Style::default().bg(COLOR_STEP2),
    );
}

fn content_width(ob: &OverlayBox) -> usize {
    ob.box_w.saturating_sub(6)
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_help_overlay(buf: &mut Buffer, size: TerminalSize) {
    if !(size.height > 10 && size.width > 44) {
        return;
    }

    let box_w = (size.width - 8).min(74);
    let box_h = 10usize;
    let ob = compute_overlay_box(size, box_w, box_h);

    fill_fullscreen_overlay(buf, size);
    fill_box_background(buf, &ob, box_h);
    draw_rounded_border(buf, &ob);
    draw_overlay_title(buf, &ob, "Help");
    draw_help_lines(buf, &ob);
}

fn draw_help_lines(buf: &mut Buffer, ob: &OverlayBox) {
    let w = content_width(ob);
    let x = ob.start_x + 3;
    let text_style = Style::default().fg(COLOR_TEXT);
    let dim_style = Style::default().fg(COLOR_DIM);

    let lines: &[(&str, Style)] = &[
        ("Enter send/steer  Shift/Alt+Enter newline", text_style),
        ("Ctrl+Y copy selection or last answer", text_style),
        ("Home/End jump transcript  Ctrl+M model settings", text_style),
        ("Wheel scroll, drag to select, release to copy", text_style),
        ("? toggle this help", dim_style),
    ];
    for (i, (text, style)) in lines.iter().enumerate() {
        draw_str(buf, x, ob.start_y + 3 + i, text, *style, w);
    }
}

// ---------------------------------------------------------------------------
// Model-settings overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_model_settings_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    if !(size.height > 14 && size.width > 56) {
        return;
    }

    let box_w = (size.width - 10).min(80);
    let box_h = 12usize;
    let ob = compute_overlay_box(size, box_w, box_h);

    fill_box_background(buf, &ob, box_h);
    draw_rounded_border(buf, &ob);

    let title = if app.runtime_supports_summary() {
        "Model / Thinking / Summary"
    } else {
        "Model / Thinking"
    };
    draw_overlay_title(buf, &ob, title);
    draw_model_settings_fields(buf, &ob, app);
}

fn field_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(COLOR_TEXT)
            .bg(COLOR_STEP6)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(COLOR_TEXT)
    }
}

fn draw_model_settings_fields(buf: &mut Buffer, ob: &OverlayBox, app: &AppState) {
    let label_style = Style::default().fg(COLOR_DIM);
    let value_w = ob.box_w.saturating_sub(16);
    let x_label = ob.start_x + 3;
    let x_value = ob.start_x + 12;

    draw_str(buf, x_label, ob.start_y + 3, "Model", label_style, 8);
    draw_str(
        buf,
        x_value,
        ob.start_y + 3,
        app.model_settings_model_value(),
        field_style(matches!(app.runtime.model_settings_field, ModelSettingsField::Model)),
        value_w,
    );
    draw_str(buf, x_label, ob.start_y + 5, "Thinking", label_style, 8);
    draw_str(
        buf,
        x_value,
        ob.start_y + 5,
        app.model_settings_effort_value(),
        field_style(matches!(app.runtime.model_settings_field, ModelSettingsField::Effort)),
        value_w,
    );

    let footer_y = if app.runtime_supports_summary() {
        draw_str(buf, x_label, ob.start_y + 7, "Summary", label_style, 8);
        draw_str(
            buf,
            x_value,
            ob.start_y + 7,
            app.model_settings_summary_value(),
            field_style(matches!(app.runtime.model_settings_field, ModelSettingsField::Summary)),
            value_w,
        );
        ob.start_y + 9
    } else {
        ob.start_y + 7
    };

    draw_str(
        buf,
        x_label,
        footer_y,
        "Tab switch field, arrows adjust, Enter apply",
        Style::default().fg(COLOR_DIM),
        content_width(ob),
    );
}

// ---------------------------------------------------------------------------
// Approval overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_approval_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    let Some(approval) = app.approval.pending.as_ref() else {
        return;
    };
    if size.width < 36 || size.height < 10 {
        return;
    }

    let inner_w = size.width.saturating_sub(12).min(92);
    let detail_lines = build_approval_detail_lines(approval, inner_w, size.height);
    let footer_text = build_approval_footer(approval);

    let box_w = inner_w;
    let box_h = (detail_lines.len() + 5).min(size.height.saturating_sub(2));
    let ob = compute_overlay_box(size, box_w, box_h);

    fill_fullscreen_overlay(buf, size);
    fill_box_background(buf, &ob, box_h);
    draw_rounded_border(buf, &ob);
    draw_approval_content(buf, &ob, approval, &detail_lines, &footer_text);
}

fn build_approval_detail_lines(
    approval: &super::approval_state::PendingApprovalRequest,
    inner_w: usize,
    height: usize,
) -> Vec<String> {
    let wrap_w = inner_w.saturating_sub(6).max(8);
    let mut detail_lines = Vec::new();
    for line in &approval.detail_lines {
        let wrapped = wrap_natural_by_cells(line, wrap_w);
        if wrapped.is_empty() {
            detail_lines.push(String::new());
        } else {
            detail_lines.extend(wrapped);
        }
    }
    if detail_lines.is_empty() {
        detail_lines.push("No additional detail provided.".to_string());
    }
    let max_body_lines = height.saturating_sub(7);
    if detail_lines.len() > max_body_lines {
        detail_lines.truncate(max_body_lines);
        if let Some(last) = detail_lines.last_mut() {
            *last = "…".to_string();
        }
    }
    detail_lines
}

fn build_approval_footer(approval: &super::approval_state::PendingApprovalRequest) -> String {
    let mut parts = vec!["y accept", "n decline"];
    if approval.can_accept_for_session {
        parts.push("s accept session");
    }
    if approval.can_cancel {
        parts.push("c cancel turn");
    }
    parts.join("  ")
}

fn draw_approval_content(
    buf: &mut Buffer,
    ob: &OverlayBox,
    approval: &super::approval_state::PendingApprovalRequest,
    detail_lines: &[String],
    footer_text: &str,
) {
    let w = content_width(ob);
    let x = ob.start_x + 3;

    draw_str(
        buf,
        x,
        ob.start_y + 1,
        &approval.title,
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        w,
    );
    draw_str(
        buf,
        x,
        ob.start_y + 2,
        &approval.method,
        Style::default().fg(COLOR_DIM),
        w,
    );
    let text_style = Style::default().fg(COLOR_TEXT);
    for (i, line) in detail_lines.iter().enumerate() {
        draw_str(buf, x, ob.start_y + 3 + i, line, text_style, w);
    }
    draw_str(
        buf,
        x,
        ob.bottom.saturating_sub(1),
        footer_text,
        Style::default().fg(COLOR_DIM),
        w,
    );
}

// ---------------------------------------------------------------------------
// Perf overlay
// ---------------------------------------------------------------------------

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
