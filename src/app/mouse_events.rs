//! Mouse and paste event handling: scroll, drag, selection, and clipboard copy.

use crossterm::event::{KeyModifiers, MouseButton, MouseEventKind};

use super::input_events::{ensure_transcript_layout, TerminalEventResult};

use super::mobile_mouse::{
    apply_mobile_mouse_scroll, parse_mobile_mouse_coords, parse_repeated_plain_mobile_pair,
};

use super::render::{compute_input_layout, normalize_pasted_text};

use super::mouse_gestures::{self, MouseHitContext};
use super::selection::normalize_selection_x;

use super::state::{AppState, ModelSettingsField};

use super::TerminalSize;
use super::MSG_TOP;
use crate::clipboard::try_copy_clipboard;

// --- Mouse event dispatch ---

pub(super) fn handle_mouse_event(
    app: &mut AppState,
    m: crossterm::event::MouseEvent,
    size: TerminalSize,
) -> TerminalEventResult {
    if app.view.claude_tasks.show || app.view.codex_panel.show {
        return TerminalEventResult::Continue { needs_draw: false };
    }
    if let Some(result) = super::native_questions::mouse(app, m) {
        return result;
    }
    if app.view.codex_queue.show || app.view.codex_goals.show {
        return TerminalEventResult::Continue { needs_draw: false };
    }
    if let Some(perf) = app.view.perf.as_mut() {
        perf.mouse_events = perf.mouse_events.saturating_add(1);
    }
    if app.view.viewport.show_help || app.view.runtime_settings.show_model_settings {
        return TerminalEventResult::Continue { needs_draw: false };
    }
    if super::native_questions::modal(app) && super::native_questions::draft(app).is_some() {
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
        norm_x: normalize_selection_x(m.column as usize, app.content_x(size)),
        clamped_y,
        clamped_line_idx: app.view.viewport.scroll_top + (clamped_y - msg_top),
        max_scroll: app.rendered_line_count().saturating_sub(msg_height),
        last_line: app.rendered_line_count().saturating_sub(1),
    };
    let mouse_changed = apply_pointer(app, m, &hit);

    TerminalEventResult::Continue {
        needs_draw: mouse_changed,
    }
}

fn apply_pointer(
    app: &mut AppState,
    m: crossterm::event::MouseEvent,
    hit: &MouseHitContext,
) -> bool {
    match m.kind {
        MouseEventKind::ScrollUp => mouse_gestures::wheel(&mut app.view.viewport, hit, true),
        MouseEventKind::ScrollDown => mouse_gestures::wheel(&mut app.view.viewport, hit, false),
        MouseEventKind::Down(MouseButton::Middle)
            if m.modifiers.contains(KeyModifiers::CONTROL)
                && m.modifiers.contains(KeyModifiers::ALT) =>
        {
            handle_mobile_middle_click(app);
            false
        }
        MouseEventKind::Down(MouseButton::Left) => {
            mouse_gestures::start(&mut app.view.viewport, hit)
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            mouse_gestures::drag(&mut app.view.viewport, hit)
        }
        MouseEventKind::Up(MouseButton::Left) => handle_mouse_up_left(app, hit),
        _ => false,
    }
}

// --- Mouse helpers ---

fn handle_mobile_middle_click(app: &mut AppState) {
    app.view.viewport.mobile_plain_pending_coords = true;
    app.view.viewport.mobile_plain_suppress_coords = false;
    app.view.viewport.mobile_plain_new_gesture = true;
    app.view.viewport.mobile_mouse_buffer.clear();
}

fn handle_mouse_up_left(app: &mut AppState, hit: &MouseHitContext) -> bool {
    let (changed, copied) = super::mouse_gestures::finish(&mut app.view.viewport, hit);
    if let Some(selection) = copied {
        let copied = app.selected_text(selection);
        if !copied.is_empty() {
            let _ = try_copy_clipboard(&copied);
        }
    }
    changed
}

// --- Paste event handling ---

pub(super) fn handle_paste_event(
    app: &mut AppState,
    pasted: String,
    size: TerminalSize,
) -> TerminalEventResult {
    if app.view.claude_tasks.show || app.view.codex_panel.show {
        return TerminalEventResult::Continue { needs_draw: false };
    }
    if let Some(perf) = app.view.perf.as_mut() {
        perf.paste_events = perf.paste_events.saturating_add(1);
    }
    if app.view.viewport.show_help {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.view.runtime_settings.show_model_settings {
        paste_model_settings(app, &pasted);
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if super::claude_question_input::paste(app, &normalize_pasted_text(&pasted), size) {
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.view.codex_goals.show {
        super::codex_goal_view::paste(app, &normalize_pasted_text(&pasted), size);
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.view.codex_queue.show {
        super::codex_queue_view::paste(app, &normalize_pasted_text(&pasted), size);
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

fn paste_model_settings(app: &mut AppState, pasted: &str) {
    if app.view.runtime_settings.model_settings_field == ModelSettingsField::Context {
        app.view.runtime_settings.context.insert(pasted.trim());
    }
    if matches!(
        app.view.runtime_settings.model_settings_field,
        ModelSettingsField::Model
    ) && !app.model_settings_has_model_choices()
    {
        let normalized = normalize_pasted_text(pasted);
        let first_line = normalized.lines().next().unwrap_or("");
        if !first_line.is_empty() {
            app.view
                .runtime_settings
                .model_settings_model_input
                .push_str(first_line);
        }
    }
}
