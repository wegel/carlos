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
};
use crate::app::render_input::InputLayout;
use crate::app::state::{AppState, ModelSettingsField};
use crate::app::text::visual_width;
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
}

// --- Status bar ---

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
    let working = app.active_turn_id.is_some();
    let ralph_mode = app.ralph_enabled();
    const RALPH_MODE_LABEL: &str = "RALPH MODE";
    let line_len = size.width.saturating_sub(1);
    let context_label = app
        .context_usage
        .map(context_usage_label)
        .unwrap_or_else(|| context_usage_placeholder_label().to_string());
    let model_label = app.runtime_settings_label();
    let dictation_label = app.dictation_status_label();
    let has_context_usage = app.context_usage.is_some();
    let has_runtime_settings = app.has_runtime_settings();
    let runtime_settings_pending = app.runtime_settings_pending();
    let ralph_label_cells = if ralph_mode {
        visual_width(RALPH_MODE_LABEL) + 1
    } else {
        0
    };
    let dictation_label_cells = dictation_label
        .as_deref()
        .map(|label| visual_width(label) + 1)
        .unwrap_or(0);
    let model_label_cells = visual_width(&model_label);
    let reserved_label_cells = context_label_reserved_cells(Some(&context_label))
        + 1
        + model_label_cells
        + ralph_label_cells
        + dictation_label_cells;
    let context_label_cells = visual_width(&context_label);
    let can_reserve_label_area = reserved_label_cells + 1 < line_len;
    let label_area_start = if can_reserve_label_area {
        line_len - reserved_label_cells
    } else {
        line_len
    };
    let anim_end = if can_reserve_label_area {
        label_area_start.saturating_sub(1)
    } else {
        line_len
    };
    let tick = animation_tick();
    let head = if anim_end > 0 {
        kitt_head_index(anim_end, tick)
    } else {
        0
    };

    if anim_end > 0 {
        if app.rewind_mode() {
            let sep = "━".repeat(anim_end);
            draw_str(
                buf,
                0,
                sep_y,
                &sep,
                Style::default().fg(COLOR_DIFF_REMOVE),
                anim_end,
            );
        } else if working {
            for x in 0..anim_end {
                let dist = head.abs_diff(x);
                draw_str(
                    buf,
                    x,
                    sep_y,
                    "━",
                    Style::default().fg(kitt_color_for_distance(dist, ralph_mode)),
                    1,
                );
            }
        } else {
            let sep = "━".repeat(anim_end);
            draw_str(
                buf,
                0,
                sep_y,
                &sep,
                Style::default().fg(if ralph_mode {
                    COLOR_GUTTER_AGENT_THINKING
                } else {
                    COLOR_GUTTER_USER
                }),
                anim_end,
            );
        }
    }

    if can_reserve_label_area && context_label_cells > 0 {
        let context_x = line_len.saturating_sub(context_label_cells);
        let model_x = context_x.saturating_sub(model_label_cells + 1);
        draw_str(
            buf,
            context_x,
            sep_y,
            &context_label,
            Style::default().fg(if has_context_usage {
                COLOR_STEP8
            } else {
                COLOR_STEP7
            }),
            context_label_cells,
        );
        draw_str(
            buf,
            model_x,
            sep_y,
            &model_label,
            Style::default().fg(if runtime_settings_pending {
                COLOR_DIFF_HUNK
            } else if has_runtime_settings {
                COLOR_STEP8
            } else {
                COLOR_STEP7
            }),
            model_label_cells,
        );
        let mut label_x = model_x;
        if let Some(label) = dictation_label.as_deref() {
            let label_w = visual_width(label);
            label_x = label_x.saturating_sub(label_w + 1);
            draw_str(
                buf,
                label_x,
                sep_y,
                label,
                Style::default()
                    .fg(COLOR_GUTTER_AGENT_COMMENTARY)
                    .add_modifier(Modifier::BOLD),
                label_w,
            );
        }
        if ralph_mode {
            let ralph_w = visual_width(RALPH_MODE_LABEL);
            let ralph_x = label_x.saturating_sub(ralph_w + 1);
            draw_str(
                buf,
                ralph_x,
                sep_y,
                RALPH_MODE_LABEL,
                Style::default()
                    .fg(COLOR_GUTTER_AGENT_THINKING)
                    .add_modifier(Modifier::BOLD),
                ralph_w,
            );
        }
    }
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
        let box_w = (size.width.saturating_sub(10)).min(80);
        let start_x = (size.width.saturating_sub(box_w)) / 2;
        let start_y = (size.height.saturating_sub(14)) / 2;
        let x = match app.runtime.model_settings_field {
            ModelSettingsField::Model => {
                start_x + 12 + visual_width(app.model_settings_model_value())
            }
            ModelSettingsField::Effort => {
                start_x + 12 + visual_width(app.model_settings_effort_value())
            }
            ModelSettingsField::Summary => {
                start_x + 12 + visual_width(app.model_settings_summary_value())
            }
            ModelSettingsField::DictationEndpoint => {
                start_x + 12 + visual_width(app.dictation_endpoint_mode_label())
            }
        };
        let y = match app.runtime.model_settings_field {
            ModelSettingsField::Model => start_y + 3,
            ModelSettingsField::Effort => start_y + 5,
            ModelSettingsField::Summary => start_y + 7,
            ModelSettingsField::DictationEndpoint => {
                if app.runtime_supports_summary() {
                    start_y + 9
                } else {
                    start_y + 7
                }
            }
        };
        (x, y)
    } else {
        (input_layout.cursor_x, input_layout.cursor_y)
    };
    (
        cursor_x.min(size.width.saturating_sub(2)),
        cursor_y.min(size.height.saturating_sub(1)),
    )
}
