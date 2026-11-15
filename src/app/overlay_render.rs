//! Overlay rendering: help, approval, model settings, and perf overlays.

#[path = "overlay_render_perf.rs"]
mod overlay_render_perf;

#[path = "overlay_render_model_settings.rs"]
mod overlay_render_model_settings;

#[path = "overlay_render_model_bridge.rs"]
mod overlay_render_model_bridge;

#[path = "question_card.rs"]
pub(super) mod question_card;

#[path = "overlay_render_claude.rs"]
mod overlay_render_claude;

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::render::{draw_str, fill_rect};

use super::text::wrap_natural_by_cells;
use super::{AppState, TerminalSize};

use crate::theme::{COLOR_DIM, COLOR_OVERLAY, COLOR_PRIMARY, COLOR_STEP2, COLOR_STEP7, COLOR_TEXT};

pub(super) use self::overlay_render_model_bridge::model_settings_cursor_position;
use self::overlay_render_model_settings::{draw_model_settings_fields, model_settings_overlay_box};
pub(super) use self::overlay_render_model_settings::{settings_cursor_position, SettingsFrame};
pub(super) use self::overlay_render_perf::draw_perf_overlay;

pub(super) struct OverlayBox {
    pub(super) start_x: usize,
    pub(super) start_y: usize,
    pub(super) box_w: usize,
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) top: usize,
    pub(super) bottom: usize,
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
    edges: (usize, usize, usize, usize),
    style: Style,
    corners: (&str, &str, &str, &str),
    horizontal: &str,
    vertical: &str,
) {
    let (left, top, right, bottom) = edges;
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
    let style = Style::default().fg(COLOR_STEP7);
    draw_box_border(
        buf,
        (ob.left, ob.top, ob.right, ob.bottom),
        style,
        ("┏", "┓", "┗", "┛"),
        "─",
        "┃",
    );
}

fn draw_overlay_title(buf: &mut Buffer, ob: &OverlayBox, title: &str) {
    let bold = Style::default()
        .fg(COLOR_PRIMARY)
        .add_modifier(Modifier::BOLD);
    draw_str(
        buf,
        ob.start_x + 3,
        ob.start_y + 1,
        title,
        bold,
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

pub(super) fn content_width(ob: &OverlayBox) -> usize {
    ob.box_w.saturating_sub(6)
}

/// Locate the current native answer caret without moving the main composer.
pub(super) fn claude_question_cursor(app: &AppState, size: TerminalSize) -> Option<(usize, usize)> {
    let draft = crate::app::native_questions::draft(app)?;
    overlay_render_claude::cursor(size, draft, app.session.approval().pending()?, app)
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_help_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    fill_fullscreen_overlay(buf, size);
    if size.width == 0 || size.height < 2 {
        return;
    }
    let lines = help_lines(size.width);
    draw_str(
        buf,
        0,
        0,
        "Help",
        Style::default().fg(COLOR_PRIMARY),
        size.width,
    );
    let height = size.height.saturating_sub(2);
    let offset = app
        .view
        .viewport
        .help_scroll
        .min(lines.len().saturating_sub(height));
    for (row, line) in lines.iter().skip(offset).take(height).enumerate() {
        draw_str(
            buf,
            0,
            row + 1,
            line,
            Style::default().fg(COLOR_TEXT),
            size.width,
        );
    }
    draw_str(
        buf,
        0,
        size.height - 1,
        "↑↓ · Esc close",
        Style::default().fg(COLOR_DIM),
        size.width,
    );
}

/// Bound stored help navigation to the lines that can actually scroll at this size.
pub(super) fn help_scroll_limit(size: TerminalSize) -> usize {
    help_lines(size.width)
        .len()
        .saturating_sub(size.height.saturating_sub(2))
}

fn help_lines(width: usize) -> Vec<String> {
    [
        "F2 tasks: agents, background work and stops (Claude/Codex)",
        "F3 switches Enter between Steer and Queue (Claude/Codex)",
        "Enter sends; during work it follows the selected mode",
        "Shift/Alt+Enter newline; F9 dictation endpoint",
        "F4 waiting questions; Ctrl+M settings",
        "Ctrl+Y copy selection or last answer",
        "Home/End jump transcript; wheel scroll; drag to copy",
        "/rename [name] changes the saved session name",
        "/tools [auto|compact|full] folds long tool output",
        "? closes this help; arrows or PgUp/PgDn scroll",
    ]
    .into_iter()
    .flat_map(|line| wrap_natural_by_cells(line, width.max(1)))
    .collect()
}

// ---------------------------------------------------------------------------
// Model-settings overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_model_settings_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    draw_settings_frame(buf, size, &overlay_render_model_bridge::frame(app));
}

/// Paint the same settings rows for an embedded editor or a daemon client's private editor.
pub(super) fn draw_settings_frame(buf: &mut Buffer, size: TerminalSize, frame: &SettingsFrame<'_>) {
    if size.height < 12 || size.width < 36 {
        return;
    }
    let ob = model_settings_overlay_box(size);
    fill_box_background(buf, &ob, ob.bottom - ob.top + 1);
    draw_rounded_border(buf, &ob);

    draw_overlay_title(buf, &ob, "Settings");
    draw_model_settings_fields(buf, &ob, frame);
}

// ---------------------------------------------------------------------------
// Approval overlay
// ---------------------------------------------------------------------------

pub(super) fn draw_approval_overlay(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    let Some(approval) = app.session.approval().pending() else {
        return;
    };
    if let Some(draft) = crate::app::native_questions::draft(app) {
        return overlay_render_claude::draw(buf, size, app, approval, draft);
    }
    draw_approval_card(
        buf,
        size,
        &approval.facts().title,
        &approval.facts().method,
        app.approval_detail_lines(approval),
        &build_approval_footer(approval),
    );
}

/// Paint the same approval box for borrowed wire facts or native session facts.
pub(super) fn draw_approval_card<'a>(
    buf: &mut Buffer,
    size: TerminalSize,
    title: &str,
    kind: &str,
    details: impl Iterator<Item = &'a str>,
    footer: &str,
) {
    if size.width < 36 || size.height < 10 {
        return;
    }
    let inner_w = size.width.saturating_sub(12).min(92);
    let detail_lines = build_approval_detail_lines(details, inner_w, size.height);
    let box_h = (detail_lines.len() + 5).min(size.height.saturating_sub(2));
    let ob = compute_overlay_box(size, inner_w, box_h);
    fill_fullscreen_overlay(buf, size);
    fill_box_background(buf, &ob, box_h);
    draw_rounded_border(buf, &ob);
    draw_approval_content(buf, &ob, title, kind, &detail_lines, footer);
}

fn build_approval_detail_lines<'a>(
    details: impl Iterator<Item = &'a str>,
    inner_w: usize,
    height: usize,
) -> Vec<String> {
    let wrap_w = inner_w.saturating_sub(6).max(8);
    let mut detail_lines = Vec::new();
    for line in details {
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

fn build_approval_footer(
    approval: &carlos_core::session::approval_state::PendingApprovalRequest,
) -> String {
    let mut parts = vec!["y accept", "n decline"];
    if approval.facts().can_accept_for_session {
        parts.push("s accept session");
    }
    if approval.facts().can_cancel {
        parts.push("c cancel turn");
    }
    parts.join("  ")
}

fn draw_approval_content(
    buf: &mut Buffer,
    ob: &OverlayBox,
    title: &str,
    kind: &str,
    detail_lines: &[String],
    footer_text: &str,
) {
    let w = content_width(ob);
    let x = ob.start_x + 3;

    draw_str(
        buf,
        x,
        ob.start_y + 1,
        title,
        Style::default()
            .fg(COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
        w,
    );
    draw_str(
        buf,
        x,
        ob.start_y + 2,
        kind,
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
