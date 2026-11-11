//! Input area, overlay layering, and cursor placement for the main draw loop.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{draw_str, fill_rect};
use crate::app::models::TerminalSize;
use crate::app::overlay_render::{
    draw_approval_overlay, draw_help_overlay, draw_model_settings_overlay, draw_perf_overlay,
    model_settings_cursor_position,
};
use crate::app::render_input::InputLayout;
use crate::app::state::AppState;
use crate::app::text::{split_at_cells, visual_width};
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_STEP2, COLOR_STEP8, COLOR_TEXT};

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
    } else {
        COLOR_GUTTER_USER
    };

    fill_rect(
        buf,
        0,
        y0,
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP2),
    );
    for row in 0..input_layout.input_height {
        let y = y0 + row;
        if !app.layout.uses_flow(size) {
            draw_str(
                buf,
                0,
                y,
                "❯",
                Style::default()
                    .fg(gutter_color)
                    .add_modifier(Modifier::BOLD),
                1,
            );
        }
        if let Some(line) = input_layout.visible_lines.get(row) {
            draw_str(
                buf,
                app.content_x(size),
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }
    draw_session_placeholder(buf, app, input_layout, size);
}

fn draw_session_placeholder(
    buf: &mut Buffer,
    app: &AppState,
    layout: &InputLayout,
    size: TerminalSize,
) {
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
            app.content_x(size) + layout.text_width - name_width,
            layout.input_top.saturating_sub(1),
            name,
            Style::default().fg(COLOR_STEP8),
            layout.text_width,
        );
    }
}

// --- Overlays ---

pub(super) fn draw_overlays(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    if app.codex.queue.show {
        crate::app::codex_queue_view::draw(buf, app, size);
    }
    if app.codex.goals.show {
        crate::app::codex_goal_view::draw(buf, app, size);
    }
    if app.viewport.show_help {
        draw_help_overlay(buf, size, app);
    }
    if let Some(perf) = app.perf.as_ref().filter(|perf| perf.show_overlay) {
        draw_perf_overlay(buf, size, perf);
    }
    if crate::app::native_questions::modal(app) {
        draw_approval_overlay(buf, size, app);
    }
    if app.runtime.show_model_settings {
        draw_model_settings_overlay(buf, size, app);
        if crate::app::native_questions::draft(app).is_none()
            && crate::app::native_questions::modal(app)
        {
            draw_approval_overlay(buf, size, app);
        }
    }
    if app.codex.panel.show {
        crate::app::codex_panel_view::draw(buf, app, size);
    }
    if app.claude.tasks.show {
        crate::app::claude_task_view::draw(buf, app, size);
    }
}

// --- Cursor ---

pub(super) fn cursor_position(
    app: &AppState,
    size: TerminalSize,
    input_layout: &InputLayout,
) -> (usize, usize) {
    let (cursor_x, cursor_y) = if app.claude.tasks.show || app.codex.panel.show {
        (0, 0)
    } else if app.runtime.show_model_settings {
        model_settings_cursor_position(app, size)
    } else if crate::app::native_questions::modal(app) {
        crate::app::overlay_render::claude_question_cursor(app, size)
            .unwrap_or((0, size.height.saturating_sub(1)))
    } else if app.codex.goals.show {
        crate::app::codex_goal_view::cursor(app, size)
    } else if app.codex.queue.show {
        crate::app::codex_queue_view::cursor(app, size)
    } else {
        (input_layout.cursor_x, input_layout.cursor_y)
    };
    (
        cursor_x.min(size.width.saturating_sub(2)),
        cursor_y.min(size.height.saturating_sub(1)),
    )
}
