//! Main rendering pipeline: transcript drawing, input area, and status bar.

use std::time::Instant;

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::context_usage::{
    context_label_reserved_cells, context_usage_label, context_usage_placeholder_label,
};
use super::overlay_render::{
    draw_approval_overlay, draw_help_overlay, draw_model_settings_overlay, draw_perf_overlay,
};
use super::render_input::InputLayout;
use super::selection::compute_selection_range;
use super::state::ModelSettingsField;
use super::text::{slice_by_cells, visual_width};
use super::transcript_render::transcript_content_width;
use super::notifications::{animation_tick, kitt_head_index};
use super::{
    AppState, RenderedLine, Role, TerminalSize, MSG_CONTENT_X, MSG_TOP,
};
use crate::theme::{
    kitt_color_for_distance, role_fg, role_gutter_fg, role_gutter_symbol, role_row_bg,
    COLOR_DIFF_HUNK, COLOR_DIFF_REMOVE, COLOR_GUTTER_AGENT_THINKING, COLOR_GUTTER_USER,
    COLOR_STEP1, COLOR_STEP2, COLOR_STEP3, COLOR_STEP6, COLOR_STEP7, COLOR_STEP8, COLOR_TEXT,
};

// --- Re-exports: items moved to render_input but accessed via super::render:: by other modules ---

pub(super) use super::render_input::{
    compute_input_layout, is_newline_enter, last_assistant_message, normalize_pasted_text,
    textarea_input_from_key,
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

fn draw_piece(buf: &mut Buffer, y: usize, x_origin: usize, max_width: usize, piece: &str, piece_style: Style, draw_x: &mut usize) {
    if piece.is_empty() || *draw_x >= x_origin + max_width { return; }
    let rem = max_width.saturating_sub(*draw_x - x_origin);
    draw_str(buf, *draw_x, y, piece, piece_style, rem);
    *draw_x += visual_width(piece);
}

fn render_segment(buf: &mut Buffer, y: usize, x_origin: usize, max_width: usize, text: &str, base_style: Style, seg_style: Style, selection: Option<(usize, usize)>, draw_x: &mut usize, col: &mut usize) {
    if *draw_x >= x_origin + max_width || text.is_empty() { return; }
    let seg_cells = visual_width(text);
    if seg_cells == 0 { return; }
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
            draw_piece(buf, y, x_origin, max_width, &selected, style.fg(COLOR_TEXT).bg(COLOR_STEP8), draw_x);
            draw_piece(buf, y, x_origin, max_width, &after, style, draw_x);
        }
    } else {
        draw_piece(buf, y, x_origin, max_width, text, style, draw_x);
    }
    *col = seg_end;
}

pub(super) fn draw_rendered_line(buf: &mut Buffer, x: usize, y: usize, max_width: usize, line: &RenderedLine, base_style: Style, selection: Option<(usize, usize)>) {
    if max_width == 0 || line.cells == 0 { return; }
    if !line.styled_segments.is_empty() { draw_str(buf, x, y, &line.text, base_style, max_width); }
    let mut draw_x = x;
    let mut col = 0usize;
    if line.styled_segments.is_empty() {
        render_segment(buf, y, x, max_width, &line.text, base_style, Style::default(), selection, &mut draw_x, &mut col);
        return;
    }
    for seg in &line.styled_segments {
        if draw_x >= x + max_width { break; }
        render_segment(buf, y, x, max_width, &seg.text, base_style, seg.style, selection, &mut draw_x, &mut col);
    }
    if col < line.cells && draw_x < x + max_width {
        let tail = slice_by_cells(&line.text, col, line.cells);
        render_segment(buf, y, x, max_width, &tail, base_style, Style::default(), selection, &mut draw_x, &mut col);
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
    let size = TerminalSize { width: area.width as usize, height: area.height as usize };
    if size.width == 0 || size.height == 0 { return; }

    // --- Layout ---
    let input_layout_started = Instant::now();
    let input_layout = compute_input_layout(app, size);
    if let Some(perf) = app.perf.as_mut() { perf.input_layout.push(input_layout_started.elapsed()); }
    let msg_height = input_layout.msg_bottom.checked_sub(MSG_TOP).map_or(0, |rows| rows + 1);
    let msg_width = transcript_content_width(size);
    sync_transcript_viewport(app, msg_height);

    // --- Draw ---
    let buf = frame.buffer_mut();
    fill_rect(buf, 0, 0, size.width, size.height, Style::default().bg(COLOR_STEP1));
    if msg_height > 0 { fill_rect(buf, 0, MSG_TOP - 1, size.width, msg_height, Style::default().bg(COLOR_STEP2)); }
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
    if app.viewport.scroll_top > max_scroll { app.viewport.scroll_top = max_scroll; }
    if app.viewport.auto_follow_bottom && max_scroll > 0 { app.viewport.scroll_top = max_scroll; }
    if msg_height == 0 || total_lines == 0 { return; }
    let end_idx = (app.viewport.scroll_top + msg_height).saturating_sub(1).min(total_lines - 1);
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

fn draw_transcript_row(buf: &mut Buffer, app: &AppState, line_idx: usize, y: usize, msg_width: usize) {
    let line_opt = app.rendered_line_at(line_idx);
    if let Some(line) = line_opt.filter(|line| !line.separator) {
        draw_str(
            buf,
            0,
            y,
            role_gutter_symbol(line.role),
            Style::default().fg(role_gutter_fg(line.role)).add_modifier(Modifier::BOLD),
            1,
        );
    }
    if msg_width == 0 { return; }

    let Some(line) = line_opt else { return; };
    fill_rect(buf, MSG_CONTENT_X, y, msg_width, 1, Style::default().bg(role_row_bg(line.role)));
    if line.separator {
        let sep = "─".repeat(msg_width);
        draw_str(buf, MSG_CONTENT_X, y, &sep, Style::default().fg(COLOR_STEP6), msg_width);
        return;
    }

    let selection = app
        .viewport
        .selection
        .and_then(|sel| compute_selection_range(sel, line_idx, line.cells))
        .map(|(start, end)| (start.min(line.cells), end.min(line.cells)));
    draw_rendered_line(buf, MSG_CONTENT_X, y, msg_width, line, transcript_line_style(line.role), selection);
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

// --- Input area ---

fn draw_input_area(buf: &mut Buffer, app: &AppState, size: TerminalSize, input_layout: &InputLayout) {
    let y0 = input_layout.input_top.saturating_sub(1);
    let gutter_color = if app.rewind_mode() {
        COLOR_DIFF_REMOVE
    } else if app.ralph_enabled() {
        COLOR_GUTTER_AGENT_THINKING
    } else {
        COLOR_GUTTER_USER
    };

    fill_rect(buf, 0, y0, size.width, input_layout.input_height, Style::default().bg(COLOR_STEP3));
    for row in 0..input_layout.input_height {
        let y = y0 + row;
        draw_str(buf, 0, y, ">", Style::default().fg(gutter_color).add_modifier(Modifier::BOLD), 1);
        if let Some(line) = input_layout.visible_lines.get(row) {
            draw_str(buf, MSG_CONTENT_X, y, line, Style::default().fg(COLOR_TEXT), input_layout.text_width);
        }
    }
}

// --- Status bar ---

fn draw_status_bar(buf: &mut Buffer, app: &AppState, size: TerminalSize, input_top: usize) {
    if input_top <= 1 || size.width == 0 {
        return;
    }
    let sep_y = input_top - 2;
    let working = app.active_turn_id.is_some();
    let ralph_mode = app.ralph_enabled();
    const RALPH_MODE_LABEL: &str = "RALPH MODE";
    let line_len = size.width.saturating_sub(1);
    let context_label = app
        .context_usage
        .map(context_usage_label)
        .unwrap_or_else(|| context_usage_placeholder_label().to_string());
    let model_label = app.runtime_settings_label();
    let has_context_usage = app.context_usage.is_some();
    let has_runtime_settings = app.has_runtime_settings();
    let runtime_settings_pending = app.runtime_settings_pending();
    let ralph_label_cells = if ralph_mode { visual_width(RALPH_MODE_LABEL) + 1 } else { 0 };
    let model_label_cells = visual_width(&model_label);
    let reserved_label_cells =
        context_label_reserved_cells(Some(&context_label)) + 1 + model_label_cells + ralph_label_cells;
    let context_label_cells = visual_width(&context_label);
    let can_reserve_label_area = reserved_label_cells + 1 < line_len;
    let label_area_start = if can_reserve_label_area { line_len - reserved_label_cells } else { line_len };
    let anim_end = if can_reserve_label_area { label_area_start.saturating_sub(1) } else { line_len };
    let tick = animation_tick();
    let head = if anim_end > 0 { kitt_head_index(anim_end, tick) } else { 0 };

    if anim_end > 0 {
        if app.rewind_mode() {
            let sep = "━".repeat(anim_end);
            draw_str(buf, 0, sep_y, &sep, Style::default().fg(COLOR_DIFF_REMOVE), anim_end);
        } else if working {
            for x in 0..anim_end {
                let dist = head.abs_diff(x);
                draw_str(buf, x, sep_y, "━", Style::default().fg(kitt_color_for_distance(dist, ralph_mode)), 1);
            }
        } else {
            let sep = "━".repeat(anim_end);
            draw_str(buf, 0, sep_y, &sep, Style::default().fg(if ralph_mode { COLOR_GUTTER_AGENT_THINKING } else { COLOR_GUTTER_USER }), anim_end);
        }
    }

    if can_reserve_label_area && context_label_cells > 0 {
        let context_x = line_len.saturating_sub(context_label_cells);
        let model_x = context_x.saturating_sub(model_label_cells + 1);
        draw_str(buf, context_x, sep_y, &context_label, Style::default().fg(if has_context_usage { COLOR_STEP8 } else { COLOR_STEP7 }), context_label_cells);
        draw_str(buf, model_x, sep_y, &model_label, Style::default().fg(if runtime_settings_pending { COLOR_DIFF_HUNK } else if has_runtime_settings { COLOR_STEP8 } else { COLOR_STEP7 }), model_label_cells);
        if ralph_mode {
            let ralph_w = visual_width(RALPH_MODE_LABEL);
            let ralph_x = model_x.saturating_sub(ralph_w + 1);
            draw_str(buf, ralph_x, sep_y, RALPH_MODE_LABEL, Style::default().fg(COLOR_GUTTER_AGENT_THINKING).add_modifier(Modifier::BOLD), ralph_w);
        }
    }
}

// --- Overlays ---

fn draw_overlays(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    if app.viewport.show_help {
        draw_help_overlay(buf, size);
    }
    if let Some(perf) = app.perf.as_ref().filter(|perf| perf.show_overlay) {
        draw_perf_overlay(buf, size, perf);
    }
    if app.runtime.show_model_settings {
        draw_model_settings_overlay(buf, size, app);
    }
    if app.approval.pending.is_some() {
        draw_approval_overlay(buf, size, app);
    }
}

// --- Cursor ---

fn cursor_position(app: &AppState, size: TerminalSize, input_layout: &InputLayout) -> (usize, usize) {
    let (cursor_x, cursor_y) = if app.approval.pending.is_some() {
        (0, size.height.saturating_sub(1))
    } else if app.runtime.show_model_settings {
        let box_w = (size.width.saturating_sub(10)).min(80);
        let start_x = (size.width.saturating_sub(box_w)) / 2;
        let start_y = (size.height.saturating_sub(12)) / 2;
        let x = match app.runtime.model_settings_field {
            ModelSettingsField::Model => start_x + 12 + visual_width(app.model_settings_model_value()),
            ModelSettingsField::Effort => start_x + 12 + visual_width(app.model_settings_effort_value()),
            ModelSettingsField::Summary => start_x + 12 + visual_width(app.model_settings_summary_value()),
        };
        let y = match app.runtime.model_settings_field {
            ModelSettingsField::Model => start_y + 3,
            ModelSettingsField::Effort => start_y + 5,
            ModelSettingsField::Summary => start_y + 7,
        };
        (x, y)
    } else {
        (input_layout.cursor_x, input_layout.cursor_y)
    };
    (cursor_x.min(size.width.saturating_sub(2)), cursor_y.min(size.height.saturating_sub(1)))
}
