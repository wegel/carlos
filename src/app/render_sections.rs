//! Input area, status bar, overlay layering, and cursor placement for the main draw loop.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{draw_str, fill_rect};
use crate::app::context_usage::{
    context_label_reserved_cells, context_usage_label, context_usage_placeholder_label,
};
use crate::app::models::TerminalSize;
use crate::app::notifications::{animation_tick, kitt_head_index};
use crate::app::overlay_render::{
    draw_approval_overlay, draw_help_overlay, draw_model_settings_overlay, draw_perf_overlay,
    model_settings_cursor_position,
};
use crate::app::render_input::InputLayout;
use crate::app::state::AppState;
use crate::app::text::{split_at_cells, visual_width};
use crate::app::MSG_CONTENT_X;
use crate::theme::{
    kitt_color_for_distance, COLOR_DIFF_HUNK, COLOR_DIFF_REMOVE, COLOR_GUTTER_AGENT_COMMENTARY,
    COLOR_GUTTER_AGENT_THINKING, COLOR_GUTTER_USER, COLOR_STEP3, COLOR_STEP7, COLOR_STEP8,
    COLOR_TEXT,
};

// --- Input area ---

pub(super) fn draw_input_area(
    buf: &mut Buffer,
    app: &AppState,
    size: TerminalSize,
    input_layout: &InputLayout,
) {
    let y0 = input_layout.input_top.saturating_sub(1);
    let gutter_color = if app.rewind_mode() {
        COLOR_DIFF_REMOVE
    } else if app.ralph_enabled() {
        COLOR_GUTTER_AGENT_THINKING
    } else {
        COLOR_GUTTER_USER
    };

    fill_rect(
        buf,
        0,
        y0,
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP3),
    );
    for row in 0..input_layout.input_height {
        let y = y0 + row;
        draw_str(
            buf,
            0,
            y,
            ">",
            Style::default()
                .fg(gutter_color)
                .add_modifier(Modifier::BOLD),
            1,
        );
        if let Some(line) = input_layout.visible_lines.get(row) {
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }
    draw_session_placeholder(buf, app, input_layout);
}

fn draw_session_placeholder(buf: &mut Buffer, app: &AppState, layout: &InputLayout) {
    if !app.input_is_empty() || app.rewind_mode() {
        return;
    }
    if let Some(name) = app.thread_name.as_deref() {
        let name = &name[..split_at_cells(name, layout.text_width)];
        let name_width = visual_width(name);
        if name_width > layout.text_width {
            return;
        }
        draw_str(
            buf,
            MSG_CONTENT_X + layout.text_width - name_width,
            layout.input_top.saturating_sub(1),
            name,
            Style::default().fg(COLOR_STEP8).add_modifier(Modifier::DIM),
            layout.text_width,
        );
    }
}

// --- Status bar ---

const RALPH_MODE_LABEL: &str = "RALPH MODE";

/// Geometry of the status separator and its reserved right-edge label area.
struct StatusGeometry {
    line_len: usize,
    anim_end: usize,
    show_labels: bool,
    ralph_mode: bool,
}

/// The label strings shown on the right edge of the status bar and their styling cues.
struct StatusLabels {
    context: String,
    model: String,
    dictation: Option<String>,
    has_context_usage: bool,
    has_runtime_settings: bool,
    runtime_settings_pending: bool,
}

pub(super) fn draw_status_bar(
    buf: &mut Buffer,
    app: &AppState,
    size: TerminalSize,
    input_top: usize,
) {
    if input_top <= 1 || size.width == 0 {
        return;
    }
    let sep_y = input_top - 2;
    let labels = collect_status_labels(app);
    let geometry = compute_status_geometry(app, size, &labels);
    draw_status_separator(buf, app, sep_y, &geometry);
    if geometry.show_labels {
        draw_status_labels(buf, sep_y, &geometry, &labels);
    }
}

fn collect_status_labels(app: &AppState) -> StatusLabels {
    StatusLabels {
        context: app
            .context_usage
            .map(context_usage_label)
            .unwrap_or_else(|| context_usage_placeholder_label().to_string()),
        model: app.runtime_settings_label(),
        dictation: app.dictation_status_label(),
        has_context_usage: app.context_usage.is_some(),
        has_runtime_settings: app.has_runtime_settings(),
        runtime_settings_pending: app.runtime_settings_pending(),
    }
}

fn compute_status_geometry(
    app: &AppState,
    size: TerminalSize,
    labels: &StatusLabels,
) -> StatusGeometry {
    let ralph_mode = app.ralph_enabled();
    let line_len = size.width.saturating_sub(1);

    let model_label_cells = visual_width(&labels.model);
    let ralph_label_cells = if ralph_mode {
        visual_width(RALPH_MODE_LABEL) + 1
    } else {
        0
    };
    let dictation_label_cells = labels
        .dictation
        .as_deref()
        .map(|label| visual_width(label) + 1)
        .unwrap_or(0);
    let reserved_label_cells = context_label_reserved_cells(Some(&labels.context))
        + 1
        + model_label_cells
        + ralph_label_cells
        + dictation_label_cells;
    let can_reserve_label_area = reserved_label_cells + 1 < line_len;
    let anim_end = if can_reserve_label_area {
        (line_len - reserved_label_cells).saturating_sub(1)
    } else {
        line_len
    };

    StatusGeometry {
        line_len,
        anim_end,
        show_labels: can_reserve_label_area && visual_width(&labels.context) > 0,
        ralph_mode,
    }
}

fn draw_status_separator(
    buf: &mut Buffer,
    app: &AppState,
    sep_y: usize,
    geometry: &StatusGeometry,
) {
    let anim_end = geometry.anim_end;
    if anim_end == 0 {
        return;
    }
    if app.rewind_mode() {
        fill_separator(buf, sep_y, anim_end, COLOR_DIFF_REMOVE);
    } else if app.active_turn_id.is_some() {
        let head = kitt_head_index(anim_end, animation_tick());
        for x in 0..anim_end {
            let dist = head.abs_diff(x);
            draw_str(
                buf,
                x,
                sep_y,
                "━",
                Style::default().fg(kitt_color_for_distance(dist, geometry.ralph_mode)),
                1,
            );
        }
    } else {
        let color = if geometry.ralph_mode {
            COLOR_GUTTER_AGENT_THINKING
        } else {
            COLOR_GUTTER_USER
        };
        fill_separator(buf, sep_y, anim_end, color);
    }
}

fn fill_separator(buf: &mut Buffer, sep_y: usize, width: usize, color: ratatui::style::Color) {
    let sep = "━".repeat(width);
    draw_str(buf, 0, sep_y, &sep, Style::default().fg(color), width);
}

fn draw_status_labels(
    buf: &mut Buffer,
    sep_y: usize,
    geometry: &StatusGeometry,
    labels: &StatusLabels,
) {
    let context_label_cells = visual_width(&labels.context);
    let model_label_cells = visual_width(&labels.model);
    let context_x = geometry.line_len.saturating_sub(context_label_cells);
    let model_x = context_x.saturating_sub(model_label_cells + 1);

    let context_color = if labels.has_context_usage {
        COLOR_STEP8
    } else {
        COLOR_STEP7
    };
    let model_color = if labels.runtime_settings_pending {
        COLOR_DIFF_HUNK
    } else if labels.has_runtime_settings {
        COLOR_STEP8
    } else {
        COLOR_STEP7
    };
    draw_label(buf, context_x, sep_y, &labels.context, context_color);
    draw_label(buf, model_x, sep_y, &labels.model, model_color);

    let mut label_x = model_x;
    if let Some(label) = labels.dictation.as_deref() {
        let label_w = visual_width(label);
        label_x = label_x.saturating_sub(label_w + 1);
        draw_label_bold(buf, label_x, sep_y, label, COLOR_GUTTER_AGENT_COMMENTARY);
    }
    if geometry.ralph_mode {
        let ralph_w = visual_width(RALPH_MODE_LABEL);
        let ralph_x = label_x.saturating_sub(ralph_w + 1);
        draw_label_bold(
            buf,
            ralph_x,
            sep_y,
            RALPH_MODE_LABEL,
            COLOR_GUTTER_AGENT_THINKING,
        );
    }
}

fn draw_label(buf: &mut Buffer, x: usize, y: usize, text: &str, color: ratatui::style::Color) {
    draw_str(
        buf,
        x,
        y,
        text,
        Style::default().fg(color),
        visual_width(text),
    );
}

fn draw_label_bold(buf: &mut Buffer, x: usize, y: usize, text: &str, color: ratatui::style::Color) {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    draw_str(buf, x, y, text, style, visual_width(text));
}

// --- Overlays ---

pub(super) fn draw_overlays(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
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

pub(super) fn cursor_position(
    app: &AppState,
    size: TerminalSize,
    input_layout: &InputLayout,
) -> (usize, usize) {
    let (cursor_x, cursor_y) = if app.approval.pending.is_some() {
        (0, size.height.saturating_sub(1))
    } else if app.runtime.show_model_settings {
        model_settings_cursor_position(app, size)
    } else {
        (input_layout.cursor_x, input_layout.cursor_y)
    };
    (
        cursor_x.min(size.width.saturating_sub(2)),
        cursor_y.min(size.height.saturating_sub(1)),
    )
}
