use std::time::Instant;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};
use ratatui_textarea::{Input as TextInput, Key as TextKey};

use super::context_usage::{
    context_label_reserved_cells, context_usage_label, context_usage_placeholder_label,
};
use super::overlay_render::{
    draw_approval_overlay, draw_help_overlay, draw_model_settings_overlay, draw_perf_overlay,
};
use super::selection::compute_selection_range;
use super::state::ModelSettingsField;
use super::text::{char_to_byte_idx, slice_by_cells, visual_width, wrap_input_line};
use super::transcript_render::transcript_content_width;
use super::{
    animation_tick, kitt_head_index, AppState, Message, RenderedLine, Role, TerminalSize,
    MSG_CONTENT_X, MSG_TOP,
};
use crate::theme::*;

#[derive(Debug, Clone)]
pub(super) struct InputLayout {
    pub(super) msg_bottom: usize, // 1-based; 0 means no transcript row is available
    pub(super) input_top: usize,  // 1-based
    pub(super) input_height: usize, // rows
    pub(super) text_width: usize, // cells available for input text
    pub(super) visible_lines: Vec<String>,
    pub(super) cursor_x: usize, // 0-based terminal column
    pub(super) cursor_y: usize, // 0-based terminal row
}

pub(super) fn input_cursor_visual_position(
    line: &str,
    cursor_col_chars: usize,
    width: usize,
) -> (usize, usize) {
    if width == 0 {
        return (0, 0);
    }

    let cursor_byte = char_to_byte_idx(line, cursor_col_chars).min(line.len());
    let prefix = &line[..cursor_byte];
    let wrapped_prefix = wrap_input_line(prefix, width);
    let row = wrapped_prefix.len().saturating_sub(1);
    let col = wrapped_prefix
        .last()
        .map(|part| visual_width(part))
        .unwrap_or(0);
    (row, col)
}

pub(super) fn textarea_input_from_code(code: KeyCode, modifiers: KeyModifiers) -> TextInput {
    let key = match code {
        KeyCode::Char(c) => TextKey::Char(c),
        KeyCode::Backspace => TextKey::Backspace,
        KeyCode::Enter => TextKey::Enter,
        KeyCode::Left => TextKey::Left,
        KeyCode::Right => TextKey::Right,
        KeyCode::Up => TextKey::Up,
        KeyCode::Down => TextKey::Down,
        KeyCode::Tab => TextKey::Tab,
        KeyCode::Delete => TextKey::Delete,
        KeyCode::Home => TextKey::Home,
        KeyCode::End => TextKey::End,
        KeyCode::PageUp => TextKey::PageUp,
        KeyCode::PageDown => TextKey::PageDown,
        KeyCode::Esc => TextKey::Esc,
        KeyCode::F(n) => TextKey::F(n),
        _ => TextKey::Null,
    };

    TextInput {
        key,
        ctrl: modifiers.contains(KeyModifiers::CONTROL),
        alt: modifiers.contains(KeyModifiers::ALT),
        shift: modifiers.contains(KeyModifiers::SHIFT),
    }
}

pub(super) fn textarea_input_from_key(k: crossterm::event::KeyEvent) -> TextInput {
    textarea_input_from_code(k.code, k.modifiers)
}

pub(super) fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn is_newline_enter(mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::SHIFT) || mods.contains(KeyModifiers::ALT)
}

pub(super) fn compute_input_layout(app: &AppState, size: TerminalSize) -> InputLayout {
    let text_width = transcript_content_width(size);
    let mut wrapped = Vec::new();
    let lines = app.input.lines();

    let (cursor_row, cursor_col_chars) = app.input.cursor();
    let mut cursor_wrapped_row = 0usize;
    let mut cursor_wrapped_col = 0usize;
    let mut cursor_set = false;

    for (row, line) in lines.iter().enumerate() {
        let wrapped_line = wrap_input_line(line, text_width);

        if row < cursor_row {
            cursor_wrapped_row += wrapped_line.len();
        } else if row == cursor_row {
            let (line_row, line_col) =
                input_cursor_visual_position(line, cursor_col_chars, text_width);
            cursor_wrapped_row += line_row;
            cursor_wrapped_col = line_col;
            cursor_set = true;
        }

        wrapped.extend(wrapped_line);
    }

    if wrapped.is_empty() {
        wrapped.push(String::new());
    }

    let mut max_input_rows = 8usize.min(size.height.max(1));
    if size.height > 1 {
        max_input_rows = max_input_rows.min(size.height - 1);
    }

    let input_height = wrapped.len().clamp(1, max_input_rows.max(1));

    if !cursor_set {
        cursor_wrapped_row = wrapped.len().saturating_sub(1);
        cursor_wrapped_col = wrapped.last().map(|line| visual_width(line)).unwrap_or(0);
    }

    let mut visible_start = wrapped.len().saturating_sub(input_height);
    if cursor_wrapped_row < visible_start {
        visible_start = cursor_wrapped_row;
    }
    if cursor_wrapped_row >= visible_start + input_height {
        visible_start = cursor_wrapped_row + 1 - input_height;
    }

    let visible_end = (visible_start + input_height).min(wrapped.len());
    let mut visible_lines = wrapped[visible_start..visible_end].to_vec();
    while visible_lines.len() < input_height {
        visible_lines.insert(0, String::new());
    }

    let input_top = size.height + 1 - input_height;
    let msg_bottom = input_top.saturating_sub(2);
    let cursor_visual_row = cursor_wrapped_row.saturating_sub(visible_start);
    let cursor_x = MSG_CONTENT_X + cursor_wrapped_col;
    let cursor_y = input_top.saturating_sub(1) + cursor_visual_row.min(input_height - 1);

    InputLayout {
        msg_bottom,
        input_top,
        input_height,
        text_width,
        visible_lines,
        cursor_x,
        cursor_y,
    }
}

pub(super) fn last_assistant_message(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, Role::Assistant) && !m.text.is_empty())
        .map(|m| m.text.as_str())
}

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

pub(super) fn draw_rendered_line(
    buf: &mut Buffer,
    x: usize,
    y: usize,
    max_width: usize,
    line: &RenderedLine,
    base_style: Style,
    selection: Option<(usize, usize)>,
) {
    if max_width == 0 || line.cells == 0 {
        return;
    }

    if !line.styled_segments.is_empty() {
        draw_str(buf, x, y, &line.text, base_style, max_width);
    }

    let mut draw_x = x;
    let mut col = 0usize;

    let render_segment =
        |text: &str, seg_style: Style, draw_x: &mut usize, col: &mut usize, buf: &mut Buffer| {
            if *draw_x >= x + max_width || text.is_empty() {
                return;
            }

            let seg_cells = visual_width(text);
            if seg_cells == 0 {
                return;
            }

            let style = base_style.patch(seg_style);
            let seg_start = *col;
            let seg_end = seg_start + seg_cells;

            let draw_piece =
                |piece: &str, piece_style: Style, draw_x: &mut usize, buf: &mut Buffer| {
                    if piece.is_empty() || *draw_x >= x + max_width {
                        return;
                    }
                    let rem = max_width.saturating_sub(*draw_x - x);
                    draw_str(buf, *draw_x, y, piece, piece_style, rem);
                    *draw_x += visual_width(piece);
                };

            if let Some((sel_start, sel_end)) = selection {
                if sel_end <= seg_start || sel_start >= seg_end {
                    draw_piece(text, style, draw_x, buf);
                } else {
                    let local_start = sel_start.saturating_sub(seg_start).min(seg_cells);
                    let local_end = sel_end.saturating_sub(seg_start).min(seg_cells);

                    let before = slice_by_cells(text, 0, local_start);
                    let selected = slice_by_cells(text, local_start, local_end);
                    let after = slice_by_cells(text, local_end, seg_cells);

                    draw_piece(&before, style, draw_x, buf);
                    draw_piece(&selected, style.fg(COLOR_TEXT).bg(COLOR_STEP8), draw_x, buf);
                    draw_piece(&after, style, draw_x, buf);
                }
            } else {
                draw_piece(text, style, draw_x, buf);
            }

            *col = seg_end;
        };

    if line.styled_segments.is_empty() {
        render_segment(&line.text, Style::default(), &mut draw_x, &mut col, buf);
        return;
    }

    for seg in &line.styled_segments {
        if draw_x >= x + max_width {
            break;
        }
        render_segment(&seg.text, seg.style, &mut draw_x, &mut col, buf);
    }

    if col < line.cells && draw_x < x + max_width {
        let tail = slice_by_cells(&line.text, col, line.cells);
        render_segment(&tail, Style::default(), &mut draw_x, &mut col, buf);
    }
}

pub(super) fn render_main_view(frame: &mut ratatui::Frame<'_>, app: &mut AppState) {
    let area = frame.area();
    let size = TerminalSize {
        width: area.width as usize,
        height: area.height as usize,
    };

    if size.width == 0 || size.height == 0 {
        return;
    }

    let input_layout_started = Instant::now();
    let input_layout = compute_input_layout(app, size);
    if let Some(perf) = app.perf.as_mut() {
        perf.input_layout.push(input_layout_started.elapsed());
    }
    let msg_top = MSG_TOP;
    let msg_bottom = input_layout.msg_bottom;
    let msg_height = if msg_bottom >= msg_top {
        msg_bottom - msg_top + 1
    } else {
        0
    };
    let msg_width = transcript_content_width(size);

    let total_lines = app.rendered_line_count();
    let max_scroll = total_lines.saturating_sub(msg_height);
    if app.scroll_top > max_scroll {
        app.scroll_top = max_scroll;
    }
    if app.auto_follow_bottom && max_scroll > 0 {
        app.scroll_top = max_scroll;
    }
    if msg_height > 0 && total_lines > 0 {
        let end_idx = (app.scroll_top + msg_height)
            .saturating_sub(1)
            .min(total_lines.saturating_sub(1));
        app.ensure_rendered_range_materialized(app.scroll_top, end_idx);
    }

    let buf = frame.buffer_mut();
    fill_rect(
        buf,
        0,
        0,
        size.width,
        size.height,
        Style::default().bg(COLOR_STEP1),
    );
    if msg_height > 0 {
        fill_rect(
            buf,
            0,
            msg_top - 1,
            size.width,
            msg_height,
            Style::default().bg(COLOR_STEP2),
        );
    }

    for i in 0..msg_height {
        let line_idx = app.scroll_top + i;
        let row_1b = msg_top + i;
        let y = row_1b - 1;

        let line_opt = app.rendered_line_at(line_idx);
        if let Some(line) = line_opt {
            if !line.separator {
                let gutter_symbol = role_gutter_symbol(line.role);
                draw_str(
                    buf,
                    0,
                    y,
                    gutter_symbol,
                    Style::default()
                        .fg(role_gutter_fg(line.role))
                        .add_modifier(Modifier::BOLD),
                    1,
                );
            }
        }

        if msg_width == 0 {
            continue;
        }

        let Some(line) = line_opt else {
            continue;
        };
        fill_rect(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            1,
            Style::default().bg(role_row_bg(line.role)),
        );
        if line.separator {
            let sep = "─".repeat(msg_width);
            draw_str(
                buf,
                MSG_CONTENT_X,
                y,
                &sep,
                Style::default().fg(COLOR_STEP6),
                msg_width,
            );
            continue;
        }

        let mut base_style = Style::default().fg(role_fg(line.role));
        if matches!(line.role, Role::Reasoning) {
            base_style = base_style.add_modifier(Modifier::DIM);
        } else if matches!(line.role, Role::Commentary) {
            base_style = base_style.add_modifier(Modifier::DIM | Modifier::ITALIC);
        }

        let selection_range = app
            .selection
            .and_then(|sel| compute_selection_range(sel, line_idx, line.cells))
            .map(|(start, end)| (start.min(line.cells), end.min(line.cells)));

        draw_rendered_line(
            buf,
            MSG_CONTENT_X,
            y,
            msg_width,
            line,
            base_style,
            selection_range,
        );
    }

    if input_layout.input_top > 1 {
        let sep_y = input_layout.input_top - 2;
        if size.width > 0 {
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
            let ralph_label_cells = if ralph_mode {
                visual_width(RALPH_MODE_LABEL) + 1
            } else {
                0
            };
            let model_label_cells = visual_width(&model_label);
            let reserved_label_cells = context_label_reserved_cells(Some(&context_label))
                + 1
                + model_label_cells
                + ralph_label_cells;
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

                if ralph_mode {
                    let ralph_label_cells = visual_width(RALPH_MODE_LABEL);
                    let ralph_x = model_x.saturating_sub(ralph_label_cells + 1);
                    draw_str(
                        buf,
                        ralph_x,
                        sep_y,
                        RALPH_MODE_LABEL,
                        Style::default()
                            .fg(COLOR_GUTTER_AGENT_THINKING)
                            .add_modifier(Modifier::BOLD),
                        ralph_label_cells,
                    );
                }
            }
        }
    }

    let ralph_mode = app.ralph_enabled();
    fill_rect(
        buf,
        0,
        input_layout.input_top.saturating_sub(1),
        size.width,
        input_layout.input_height,
        Style::default().bg(COLOR_STEP3),
    );
    for i in 0..input_layout.input_height {
        let y = input_layout.input_top.saturating_sub(1) + i;
        let input_gutter_color = if app.rewind_mode() {
            COLOR_DIFF_REMOVE
        } else if ralph_mode {
            COLOR_GUTTER_AGENT_THINKING
        } else {
            COLOR_GUTTER_USER
        };
        draw_str(
            buf,
            0,
            y,
            ">",
            Style::default()
                .fg(input_gutter_color)
                .add_modifier(Modifier::BOLD),
            1,
        );
        if let Some(line) = input_layout.visible_lines.get(i) {
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

    if app.show_help {
        draw_help_overlay(buf, size);
    }
    if let Some(perf) = app.perf.as_ref() {
        if perf.show_overlay {
            draw_perf_overlay(buf, size, perf);
        }
    }
    if app.runtime.show_model_settings {
        draw_model_settings_overlay(buf, size, app);
    }
    if app.approval.pending.is_some() {
        draw_approval_overlay(buf, size, app);
    }

    let (cursor_x, cursor_y) = if app.approval.pending.is_some() {
        (0, size.height.saturating_sub(1))
    } else if app.runtime.show_model_settings {
        let box_w = (size.width.saturating_sub(10)).min(80);
        let box_h = 12usize;
        let start_x = (size.width.saturating_sub(box_w)) / 2;
        let start_y = (size.height.saturating_sub(box_h)) / 2;
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
    let cursor_x = cursor_x.min(size.width.saturating_sub(2));
    let cursor_y = cursor_y.min(size.height.saturating_sub(1));
    frame.set_cursor_position((cursor_x as u16, cursor_y as u16));
}
