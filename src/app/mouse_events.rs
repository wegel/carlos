//! Mouse and paste event handling: scroll, drag, selection, and clipboard copy.

use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

use super::input_events::{ensure_transcript_layout, TerminalEventResult};
use super::mobile_mouse::{
    apply_mobile_mouse_scroll, parse_mobile_mouse_coords, parse_repeated_plain_mobile_pair,
};
use super::render::{compute_input_layout, normalize_pasted_text};
use super::selection::{decide_mouse_drag_mode, normalize_selection_x, shift_selection_focus, MouseDragMode, Selection};
use super::state::{AppState, ModelSettingsField};
use super::TerminalSize;
use super::MSG_TOP;
use crate::clipboard::try_copy_clipboard;

// --- Types ---

pub(super) struct MouseHitContext {
    pub(super) in_messages: bool,
    pub(super) norm_x: usize,
    pub(super) clamped_y: usize,
    pub(super) clamped_line_idx: usize,
    pub(super) max_scroll: usize,
}

// --- Mouse event dispatch ---

pub(super) fn handle_mouse_event(
    app: &mut AppState,
    m: crossterm::event::MouseEvent,
    size: TerminalSize,
) -> TerminalEventResult {
    if let Some(perf) = app.perf.as_mut() {
        perf.mouse_events = perf.mouse_events.saturating_add(1);
    }
    if app.viewport.show_help || app.runtime.show_model_settings {
        return TerminalEventResult::Continue { needs_draw: false };
    }
    ensure_transcript_layout(app, size);

    let msg_top = MSG_TOP;
    let msg_bottom = compute_input_layout(app, size).msg_bottom;
    if msg_bottom < msg_top {
        return TerminalEventResult::Continue { needs_draw: false };
    }

    let row1 = m.row as usize + 1;
    let clamped_y = row1.clamp(msg_top, msg_bottom);
    let msg_height = msg_bottom - msg_top + 1;
    let hit = MouseHitContext {
        in_messages: row1 >= msg_top && row1 <= msg_bottom,
        norm_x: normalize_selection_x(m.column as usize),
        clamped_y,
        clamped_line_idx: app.viewport.scroll_top + (clamped_y - msg_top),
        max_scroll: app.rendered_line_count().saturating_sub(msg_height),
    };

    let mouse_changed = match m.kind {
        MouseEventKind::ScrollUp => handle_mouse_wheel(app, &hit, true),
        MouseEventKind::ScrollDown => handle_mouse_wheel(app, &hit, false),
        MouseEventKind::Down(MouseButton::Middle)
            if m.modifiers.contains(KeyModifiers::CONTROL)
                && m.modifiers.contains(KeyModifiers::ALT) =>
        {
            handle_mobile_middle_click(app);
            false
        }
        MouseEventKind::Down(MouseButton::Left) => handle_mouse_down_left(app, &hit),
        MouseEventKind::Drag(MouseButton::Left) => handle_mouse_drag_left(app, &hit),
        MouseEventKind::Up(MouseButton::Left) => handle_mouse_up_left(app, &hit),
        _ => false,
    };

    TerminalEventResult::Continue {
        needs_draw: mouse_changed,
    }
}

// --- Mouse helpers ---

fn handle_mouse_wheel(app: &mut AppState, hit: &MouseHitContext, is_up: bool) -> bool {
    let prev_scroll = app.viewport.scroll_top;
    let prev_follow = app.viewport.auto_follow_bottom;
    let natural_up = is_up != app.viewport.scroll_inverted;
    if natural_up {
        app.viewport.scroll_top = app.viewport.scroll_top.saturating_sub(3);
    } else {
        app.viewport.scroll_top =
            (app.viewport.scroll_top.saturating_add(3)).min(hit.max_scroll);
    }
    app.sync_auto_follow_bottom(hit.max_scroll);
    let scroll_delta = app.viewport.scroll_top as isize - prev_scroll as isize;
    if scroll_delta != 0 && app.viewport.mouse_drag_mode != MouseDragMode::Scroll {
        let max_line_idx = app.rendered_line_count().saturating_sub(1);
        if let Some(sel) = app.viewport.selection.as_mut() {
            if sel.dragging {
                shift_selection_focus(sel, scroll_delta, max_line_idx);
            }
        }
    }
    app.viewport.scroll_top != prev_scroll || app.viewport.auto_follow_bottom != prev_follow
}

fn handle_mobile_middle_click(app: &mut AppState) {
    app.viewport.mobile_plain_pending_coords = true;
    app.viewport.mobile_plain_suppress_coords = false;
    app.viewport.mobile_plain_new_gesture = true;
    app.viewport.mobile_mouse_buffer.clear();
}

fn handle_mouse_down_left(app: &mut AppState, hit: &MouseHitContext) -> bool {
    app.viewport.mouse_drag_mode = MouseDragMode::Undecided;
    app.viewport.mouse_drag_last_row = hit.clamped_y;
    if hit.in_messages {
        app.viewport.selection = Some(Selection {
            anchor_x: hit.norm_x,
            anchor_line_idx: hit.clamped_line_idx,
            focus_x: hit.norm_x,
            focus_line_idx: hit.clamped_line_idx,
            dragging: true,
        });
        true
    } else {
        false
    }
}

fn handle_mouse_drag_left(app: &mut AppState, hit: &MouseHitContext) -> bool {
    let Some(sel) = app.viewport.selection.as_mut() else {
        return false;
    };
    if !sel.dragging {
        return false;
    }
    if app.viewport.mouse_drag_mode == MouseDragMode::Undecided {
        app.viewport.mouse_drag_mode = decide_mouse_drag_mode(
            sel.anchor_x,
            app.viewport.mouse_drag_last_row,
            hit.norm_x,
            hit.clamped_y,
        );
    }

    match app.viewport.mouse_drag_mode {
        MouseDragMode::Scroll => {
            let prev_scroll = app.viewport.scroll_top;
            let prev_follow = app.viewport.auto_follow_bottom;
            if hit.clamped_y > app.viewport.mouse_drag_last_row {
                app.viewport.scroll_top = app
                    .viewport
                    .scroll_top
                    .saturating_sub(hit.clamped_y - app.viewport.mouse_drag_last_row);
            } else if hit.clamped_y < app.viewport.mouse_drag_last_row {
                app.viewport.scroll_top = app
                    .viewport
                    .scroll_top
                    .saturating_add(app.viewport.mouse_drag_last_row - hit.clamped_y);
            }
            app.viewport.scroll_top = app.viewport.scroll_top.min(hit.max_scroll);
            app.sync_auto_follow_bottom(hit.max_scroll);
            app.viewport.mouse_drag_last_row = hit.clamped_y;
            app.viewport.scroll_top != prev_scroll
                || app.viewport.auto_follow_bottom != prev_follow
        }
        MouseDragMode::Select | MouseDragMode::Undecided => {
            let prev_focus_x = sel.focus_x;
            let prev_focus_idx = sel.focus_line_idx;
            sel.focus_x = hit.norm_x;
            sel.focus_line_idx = hit.clamped_line_idx;
            sel.focus_x != prev_focus_x || sel.focus_line_idx != prev_focus_idx
        }
    }
}

fn handle_mouse_up_left(app: &mut AppState, hit: &MouseHitContext) -> bool {
    let mut mouse_changed = false;
    let mut selection_to_copy = None;
    if let Some(sel) = app.viewport.selection.as_mut() {
        if sel.dragging {
            let prev_focus_x = sel.focus_x;
            let prev_focus_idx = sel.focus_line_idx;
            sel.focus_x = hit.norm_x;
            sel.focus_line_idx = hit.clamped_line_idx;
            sel.dragging = false;
            mouse_changed = sel.focus_x != prev_focus_x
                || sel.focus_line_idx != prev_focus_idx
                || !sel.dragging;

            if app.viewport.mouse_drag_mode == MouseDragMode::Scroll {
                app.viewport.selection = None;
            } else {
                selection_to_copy = Some(*sel);
            }
        }
    }
    if let Some(selection_to_copy) = selection_to_copy {
        let copied = app.selected_text(selection_to_copy);
        if !copied.is_empty() {
            let _ = try_copy_clipboard(&copied);
        }
    }
    app.viewport.mouse_drag_mode = MouseDragMode::Undecided;
    mouse_changed
}

// --- Paste event handling ---

pub(super) fn handle_paste_event(app: &mut AppState, pasted: String) -> TerminalEventResult {
    if let Some(perf) = app.perf.as_mut() {
        perf.paste_events = perf.paste_events.saturating_add(1);
    }
    if app.viewport.show_help {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.runtime.show_model_settings {
        if matches!(app.runtime.model_settings_field, ModelSettingsField::Model)
            && !app.model_settings_has_model_choices()
        {
            let normalized = normalize_pasted_text(&pasted);
            let first_line = normalized.lines().next().unwrap_or("");
            if !first_line.is_empty() {
                app.runtime.model_settings_model_input.push_str(first_line);
            }
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if let Some((_, y)) = parse_repeated_plain_mobile_pair(&pasted) {
        apply_mobile_mouse_scroll(app, y);
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.input_is_empty() {
        if let Some((_, y)) = parse_mobile_mouse_coords(&pasted) {
            apply_mobile_mouse_scroll(app, y);
            return TerminalEventResult::Continue { needs_draw: true };
        }
    }
    let normalized = normalize_pasted_text(&pasted);
    if !normalized.is_empty() {
        app.input_insert_text(normalized);
        return TerminalEventResult::Continue { needs_draw: true };
    }
    TerminalEventResult::Continue { needs_draw: false }
}
