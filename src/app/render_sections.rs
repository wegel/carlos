//! Input area, overlay layering, and cursor placement for the main draw loop.

use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};

use super::{draw_str, fill_rect};

use crate::app::overlay_render::{
    draw_approval_overlay, draw_help_overlay, draw_model_settings_overlay, draw_perf_overlay,
    model_settings_cursor_position,
};

use crate::app::render_input::InputLayout;
use crate::app::state::AppState;
use crate::app::text::{split_at_cells, visual_width};

use crate::app::view_models::TerminalSize;
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_STEP2, COLOR_STEP8, COLOR_TEXT};

// --- Input area ---

pub(super) fn draw_input_area(
    buf: &mut Buffer,
    app: &AppState,
    size: TerminalSize,
    input_layout: &InputLayout,
) {
    let name = (app.input_is_empty() && !app.rewind_mode())
        .then(|| app.session.thread_name().as_deref())
        .flatten();
    draw_composer(
        buf,
        size,
        input_layout,
        app.content_x(size),
        app.rewind_mode(),
        name,
    );
}

/// Paint one local composer without reading or mutating a core session.
pub(in crate::app) fn draw_composer(
    buf: &mut Buffer,
    size: TerminalSize,
    input_layout: &InputLayout,
    content_x: usize,
    rewind: bool,
    name: Option<&str>,
) {
    let y0 = input_layout.input_top.saturating_sub(1);
    let gutter_color = if rewind {
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
        if content_x != 1 {
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
                content_x,
                y,
                line,
                Style::default().fg(COLOR_TEXT),
                input_layout.text_width,
            );
        }
    }
    draw_session_placeholder(buf, input_layout, content_x, name);
}

fn draw_session_placeholder(
    buf: &mut Buffer,
    layout: &InputLayout,
    content_x: usize,
    name: Option<&str>,
) {
    let Some(name) = name else { return };
    let name = &name[..split_at_cells(name, layout.text_width)];
    let name_width = visual_width(name);
    if name_width <= layout.text_width {
        draw_str(
            buf,
            content_x + layout.text_width - name_width,
            layout.input_top.saturating_sub(1),
            name,
            Style::default().fg(COLOR_STEP8),
            layout.text_width,
        );
    }
}

// --- Overlays ---

pub(super) fn draw_overlays(buf: &mut Buffer, size: TerminalSize, app: &AppState) {
    if app.view.codex_queue.show {
        crate::app::codex_queue_view::draw(buf, app, size);
    }
    if app.view.codex_goals.show {
        crate::app::codex_goal_view::draw(buf, app, size);
    }
    if app.view.viewport.show_help {
        draw_help_overlay(buf, size, app);
    }
    if let Some(perf) = app.view.perf.as_ref().filter(|perf| perf.show_overlay) {
        draw_perf_overlay(buf, size, perf);
    }
    if crate::app::native_questions::modal(app) {
        draw_approval_overlay(buf, size, app);
    }
    if app.view.runtime_settings.show_model_settings {
        draw_model_settings_overlay(buf, size, app);
        if crate::app::native_questions::draft(app).is_none()
            && crate::app::native_questions::modal(app)
        {
            draw_approval_overlay(buf, size, app);
        }
    }
    if app.view.codex_panel.show {
        crate::app::codex_panel_view::draw(buf, app, size);
    }
    if app.view.claude_tasks.show {
        crate::app::claude_task_view::draw(buf, app, size);
    }
}

// --- Cursor ---

pub(super) fn cursor_position(
    app: &AppState,
    size: TerminalSize,
    input_layout: &InputLayout,
) -> (usize, usize) {
    let (cursor_x, cursor_y) = if app.view.claude_tasks.show || app.view.codex_panel.show {
        (0, 0)
    } else if app.view.runtime_settings.show_model_settings {
        model_settings_cursor_position(app, size)
    } else if crate::app::native_questions::modal(app) {
        crate::app::overlay_render::claude_question_cursor(app, size)
            .unwrap_or((0, size.height.saturating_sub(1)))
    } else if app.view.codex_goals.show {
        crate::app::codex_goal_view::cursor(app, size)
    } else if app.view.codex_queue.show {
        crate::app::codex_queue_view::cursor(app, size)
    } else {
        (input_layout.cursor_x, input_layout.cursor_y)
    };
    (
        cursor_x.min(size.width.saturating_sub(2)),
        cursor_y.min(size.height.saturating_sub(1)),
    )
}
