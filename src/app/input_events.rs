use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};

use super::mobile_mouse::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, parse_mobile_mouse_coords,
    parse_repeated_plain_mobile_pair, take_mobile_mouse_buffer, MobileMouseConsume,
};
use super::notifications::{is_ctrl_char, is_key_press_like, is_perf_toggle_key};
use super::render::{
    compute_input_layout, is_newline_enter, last_assistant_message, normalize_pasted_text,
};
use super::selection::{
    decide_mouse_drag_mode, normalize_selection_x, shift_selection_focus, MouseDragMode, Selection,
};
use super::state::{AppState, ApprovalChoice, ModelSettingsField};
use super::transcript_render::transcript_content_width;
use super::{persist_runtime_defaults, TerminalSize, MSG_TOP};
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};
use crate::backend::BackendClient;
use crate::protocol::{
    params_turn_interrupt, params_turn_start, params_turn_steer,
};

pub(super) enum TerminalEventResult {
    Quit,
    Continue { needs_draw: bool },
}

pub(super) fn trace_terminal_event(ev: &Event) {
    static TRACE_FILE: OnceLock<Option<Mutex<std::fs::File>>> = OnceLock::new();
    let trace = TRACE_FILE.get_or_init(|| {
        let path = std::env::var_os("CARLOS_EVENT_TRACE")?;
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
            .map(Mutex::new)
    });
    let Some(file_mutex) = trace else {
        return;
    };
    let Ok(mut file) = file_mutex.lock() else {
        return;
    };
    let _ = match ev {
        Event::Key(k) => writeln!(
            &mut *file,
            "key code={:?} mods={:?} kind={:?}",
            k.code, k.modifiers, k.kind
        ),
        Event::Mouse(m) => writeln!(
            &mut *file,
            "mouse kind={:?} col={} row={} mods={:?}",
            m.kind, m.column, m.row, m.modifiers
        ),
        Event::Paste(p) => writeln!(&mut *file, "paste bytes={} {:?}", p.len(), p),
        Event::Resize(w, h) => writeln!(&mut *file, "resize {} {}", w, h),
        _ => writeln!(&mut *file, "event {:?}", ev),
    };
}

pub(super) fn is_mobile_mouse_key_candidate(
    app: &AppState,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if app.viewport.mobile_plain_pending_coords || app.viewport.mobile_plain_suppress_coords {
        return matches!(code, KeyCode::Char(ch) if ch.is_ascii_digit() || ch == ';');
    }

    if !app.viewport.mobile_mouse_buffer.is_empty() {
        return matches!(code, KeyCode::Char(_));
    }

    let KeyCode::Char(ch) = code else {
        return false;
    };
    if ch == '<' {
        return true;
    }

    if modifiers.contains(KeyModifiers::ALT) {
        return ch == '['
            || ch == '<'
            || ch == ';'
            || ch == 'M'
            || ch == 'm'
            || ch.is_ascii_digit();
    }

    false
}

pub(super) fn submit_turn_text(client: &dyn BackendClient, app: &mut AppState, text: String) {
    if text.trim().is_empty() {
        return;
    }

    if let Some(turn_id) = app.active_turn_id.clone() {
        let params = params_turn_steer(&app.thread_id, &turn_id, &text);
        match client.call("turn/steer", params, Duration::from_secs(10)) {
            Ok(_) => app.set_status("sent steer"),
            Err(e) => app.set_status(format!("{e}")),
        }
    } else {
        let (model, effort, summary) = app.next_turn_runtime_settings();
        let params = params_turn_start(
            &app.thread_id,
            &text,
            model.as_deref(),
            effort.as_deref(),
            summary.as_deref(),
        );
        match client.call("turn/start", params, Duration::from_secs(10)) {
            Ok(_) => {
                app.mark_runtime_settings_applied();
                app.set_status("sent turn");
            }
            Err(e) => app.set_status(format!("{e}")),
        }
    }
}

fn respond_to_pending_approval(
    client: &dyn BackendClient,
    app: &mut AppState,
    choice: ApprovalChoice,
) {
    let Some(pending) = app.approval.pending.clone() else {
        return;
    };
    let Some(result) = pending.response_for_choice(choice) else {
        app.set_status("approval choice not available");
        return;
    };

    match client.respond(&pending.request_id, result) {
        Ok(_) => {
            app.clear_pending_approval();
            let status = match choice {
                ApprovalChoice::Accept => "approval accepted",
                ApprovalChoice::AcceptForSession => "approval accepted for session",
                ApprovalChoice::Decline => "approval declined",
                ApprovalChoice::Cancel => "approval canceled",
            };
            app.set_status(status);
        }
        Err(err) => app.set_status(format!("approval reply failed: {err}")),
    }
}

fn hidden_user_message_idx(app: &AppState) -> Option<usize> {
    if app.rewind_mode() {
        app.rewind_selected_message_idx()
    } else {
        None
    }
}

pub(super) fn ensure_transcript_layout(app: &mut AppState, size: TerminalSize) {
    let render_started = Instant::now();
    app.ensure_rendered_lines(transcript_content_width(size), hidden_user_message_idx(app));
    if let Some(perf) = app.perf.as_mut() {
        perf.transcript_render.push(render_started.elapsed());
    }
}

pub(super) fn handle_terminal_event(
    client: &dyn BackendClient,
    app: &mut AppState,
    ev: Event,
    size: TerminalSize,
) -> TerminalEventResult {
    trace_terminal_event(&ev);
    match ev {
        Event::Key(k) => handle_key_event(client, app, k, size),
        Event::Mouse(m) => handle_mouse_event(app, m, size),
        Event::Paste(pasted) => handle_paste_event(app, pasted),
        Event::Resize(_, _) => {
            if let Some(perf) = app.perf.as_mut() {
                perf.resize_events = perf.resize_events.saturating_add(1);
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        _ => TerminalEventResult::Continue { needs_draw: false },
    }
}

fn handle_key_event(
    client: &dyn BackendClient,
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
    size: TerminalSize,
) -> TerminalEventResult {
    if let Some(perf) = app.perf.as_mut() {
        perf.mark_key_kind(k.kind);
    }
    if !is_key_press_like(k.kind) {
        return TerminalEventResult::Continue { needs_draw: false };
    }
    if let Some(perf) = app.perf.as_mut() {
        perf.mark_key_event();
    }
    if is_perf_toggle_key(k.code, k.modifiers) {
        if let Some(perf) = app.perf.as_mut() {
            perf.toggle_overlay();
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if matches!(k.code, KeyCode::F(6)) && k.modifiers.is_empty() {
        app.viewport.scroll_inverted = !app.viewport.scroll_inverted;
        return TerminalEventResult::Continue { needs_draw: true };
    }
    let now = Instant::now();
    app.expire_esc_chord(now);
    if k.code != KeyCode::Esc {
        app.reset_esc_chord();
    }
    if app.viewport.show_help {
        match (k.code, k.modifiers) {
            (code, mods) if is_ctrl_char(code, mods, 'c') => return TerminalEventResult::Quit,
            (KeyCode::Esc, _) | (KeyCode::Char('?'), _) => {
                app.viewport.show_help = false;
            }
            _ => {}
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.approval.pending.is_some() {
        match (k.code, k.modifiers) {
            (code, mods) if is_ctrl_char(code, mods, 'c') => return TerminalEventResult::Quit,
            (KeyCode::Char('y'), mods) | (KeyCode::Char('a'), mods) if mods.is_empty() => {
                respond_to_pending_approval(client, app, ApprovalChoice::Accept);
            }
            (KeyCode::Char('s'), mods) if mods.is_empty() => {
                respond_to_pending_approval(client, app, ApprovalChoice::AcceptForSession);
            }
            (KeyCode::Char('n'), mods) if mods.is_empty() => {
                respond_to_pending_approval(client, app, ApprovalChoice::Decline);
            }
            (KeyCode::Char('c'), mods) if mods.is_empty() => {
                respond_to_pending_approval(client, app, ApprovalChoice::Cancel);
            }
            _ => {}
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.runtime.show_model_settings {
        match (k.code, k.modifiers) {
            (code, mods) if is_ctrl_char(code, mods, 'c') => return TerminalEventResult::Quit,
            (KeyCode::Esc, _) => app.close_model_settings(),
            (KeyCode::Tab, _) | (KeyCode::Down, _) => app.model_settings_move_field(true),
            (KeyCode::BackTab, _) | (KeyCode::Up, _) => app.model_settings_move_field(false),
            (KeyCode::Left, _) => match app.runtime.model_settings_field {
                ModelSettingsField::Model => app.model_settings_cycle_model(-1),
                ModelSettingsField::Effort => app.model_settings_cycle_effort(-1),
                ModelSettingsField::Summary => app.model_settings_cycle_summary(-1),
            },
            (KeyCode::Right, _) => match app.runtime.model_settings_field {
                ModelSettingsField::Model => app.model_settings_cycle_model(1),
                ModelSettingsField::Effort => app.model_settings_cycle_effort(1),
                ModelSettingsField::Summary => app.model_settings_cycle_summary(1),
            },
            (KeyCode::Backspace, _)
                if matches!(app.runtime.model_settings_field, ModelSettingsField::Model)
                    && !app.model_settings_has_model_choices() =>
            {
                app.model_settings_backspace();
            }
            (KeyCode::Enter, _) => {
                let defaults = app.apply_model_settings();
                if let Err(err) = persist_runtime_defaults(&defaults) {
                    app.set_status(format!("saved for next turn; default save failed: {err}"));
                }
            }
            (KeyCode::Char(ch), mods)
                if !mods.contains(KeyModifiers::CONTROL)
                    && !mods.contains(KeyModifiers::ALT)
                    && matches!(app.runtime.model_settings_field, ModelSettingsField::Model) =>
            {
                if !app.model_settings_has_model_choices() {
                    app.model_settings_insert_char(ch);
                }
            }
            _ => {}
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }

    if is_mobile_mouse_key_candidate(app, k.code, k.modifiers) {
        if let KeyCode::Char(ch) = k.code {
            let was_plain_capture = app.viewport.mobile_plain_pending_coords;
            match consume_mobile_mouse_char(app, ch) {
                MobileMouseConsume::PassThrough => {}
                MobileMouseConsume::Consumed => {
                    return TerminalEventResult::Continue { needs_draw: true };
                }
                MobileMouseConsume::Emit(text) => {
                    if !text.is_empty() && !was_plain_capture {
                        app.input_insert_text(text);
                    }
                    return TerminalEventResult::Continue { needs_draw: true };
                }
            }
        }
    } else if let Some(text) = take_mobile_mouse_buffer(app) {
        if !text.is_empty()
            && !app.viewport.mobile_plain_pending_coords
            && !app.viewport.mobile_plain_suppress_coords
        {
            app.input_insert_text(text);
        }
        app.viewport.mobile_plain_pending_coords = false;
        app.viewport.mobile_plain_suppress_coords = false;
    }

    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => TerminalEventResult::Quit,
        (KeyCode::Char('m'), KeyModifiers::CONTROL) => {
            app.toggle_model_settings();
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
            if let Err(e) = app.request_ralph_toggle() {
                app.set_status(format!("ralph: {e}"));
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
            if let Some(sel) = app.viewport.selection {
                let copied = app.selected_text(sel);
                if !copied.is_empty() {
                    let _ = try_copy_clipboard(&copied);
                }
            } else if let Some(text) = last_assistant_message(&app.messages) {
                let backend = try_copy_clipboard(text);
                app.set_status(format!(
                    "copied last assistant message ({})",
                    clipboard_backend_label(backend)
                ));
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
            app.viewport.selection = None;
            app.viewport.mouse_drag_mode = MouseDragMode::Undecided;
            app.set_status("selection cleared");
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Esc, mods) if mods.is_empty() => handle_escape_key(client, app, size, now),
        (KeyCode::Home, _) if app.input_is_empty() => {
            ensure_transcript_layout(app, size);
            app.viewport.auto_follow_bottom = false;
            app.viewport.scroll_top = 0;
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::End, _) if app.input_is_empty() => {
            ensure_transcript_layout(app, size);
            app.viewport.auto_follow_bottom = true;
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Up, _) => {
            if app.navigate_input_history_up() {
                app.align_rewind_scroll_to_selected_prompt(size);
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Down, _) => {
            if app.navigate_input_history_down() {
                app.align_rewind_scroll_to_selected_prompt(size);
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::PageUp, _) => {
            ensure_transcript_layout(app, size);
            let msg_bottom = compute_input_layout(app, size).msg_bottom;
            let msg_height = if msg_bottom >= MSG_TOP {
                msg_bottom - MSG_TOP + 1
            } else {
                0
            };
            let max_scroll = app.rendered_line_count().saturating_sub(msg_height);
            app.viewport.scroll_top = app.viewport.scroll_top.saturating_sub(10);
            app.sync_auto_follow_bottom(max_scroll);
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::PageDown, _) => {
            ensure_transcript_layout(app, size);
            let msg_bottom = compute_input_layout(app, size).msg_bottom;
            let msg_height = if msg_bottom >= MSG_TOP {
                msg_bottom - MSG_TOP + 1
            } else {
                0
            };
            let max_scroll = app.rendered_line_count().saturating_sub(msg_height);
            app.viewport.scroll_top = app.viewport.scroll_top.saturating_add(10).min(max_scroll);
            app.sync_auto_follow_bottom(max_scroll);
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Enter, mods) if is_newline_enter(mods) => {
            app.input_apply_key(k);
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Enter, _) => {
            if app.input_is_empty() {
                return TerminalEventResult::Continue { needs_draw: true };
            }

            let rewind_target_idx = if app.rewind_mode() {
                app.rewind_selected_message_idx()
            } else {
                None
            };
            let text = app.input_text();
            app.clear_rewind_mode_state();
            app.rewind_fork_from_message_idx(rewind_target_idx);
            app.push_input_history(&text);
            app.clear_input();
            app.viewport.selection = None;
            app.mark_user_turn_submitted();
            submit_turn_text(client, app, text);
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Char('?'), _) => {
            if app.input_is_empty() {
                app.viewport.show_help = true;
            } else {
                app.input_apply_key(k);
            }
            TerminalEventResult::Continue { needs_draw: true }
        }
        _ => {
            app.input_apply_key(k);
            TerminalEventResult::Continue { needs_draw: true }
        }
    }
}

fn handle_escape_key(
    client: &dyn BackendClient,
    app: &mut AppState,
    size: TerminalSize,
    now: Instant,
) -> TerminalEventResult {
    if app.rewind_mode() {
        app.exit_rewind_mode_restore();
        app.reset_esc_chord();
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if let Some(turn_id) = app.active_turn_id.clone() {
        app.reset_esc_chord();
        let params = params_turn_interrupt(&app.thread_id, &turn_id);
        match client.call("turn/interrupt", params, Duration::from_secs(10)) {
            Ok(_) => {
                app.append_turn_interrupted_marker();
                app.set_status("interrupt requested");
            }
            Err(e) => app.set_status(format!("{e}")),
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.register_escape_press(now) {
        app.viewport.selection = None;
        app.viewport.mouse_drag_mode = MouseDragMode::Undecided;
        if app.input_is_empty() && app.has_pending_ralph_continuation() {
            app.disable_ralph_mode();
        } else if app.input_is_empty() {
            app.enter_rewind_mode();
            app.align_rewind_scroll_to_selected_prompt(size);
        } else {
            app.clear_input();
        }
    }
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_mouse_event(
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
    let in_messages = row1 >= msg_top && row1 <= msg_bottom;
    let norm_x = normalize_selection_x(m.column as usize);
    let clamped_y = row1.clamp(msg_top, msg_bottom);
    let clamped_line_idx = app.viewport.scroll_top + (clamped_y - msg_top);
    let msg_height = msg_bottom - msg_top + 1;
    let max_scroll = app.rendered_line_count().saturating_sub(msg_height);
    let mut mouse_changed = false;

    match m.kind {
        MouseEventKind::ScrollUp => {
            let prev_scroll = app.viewport.scroll_top;
            let prev_follow = app.viewport.auto_follow_bottom;
            if app.viewport.scroll_inverted {
                app.viewport.scroll_top = (app.viewport.scroll_top.saturating_add(3)).min(max_scroll);
            } else {
                app.viewport.scroll_top = app.viewport.scroll_top.saturating_sub(3);
            }
            app.sync_auto_follow_bottom(max_scroll);
            let scroll_delta = app.viewport.scroll_top as isize - prev_scroll as isize;
            if scroll_delta != 0 && app.viewport.mouse_drag_mode != MouseDragMode::Scroll {
                let max_line_idx = app.rendered_line_count().saturating_sub(1);
                if let Some(sel) = app.viewport.selection.as_mut() {
                    if sel.dragging {
                        shift_selection_focus(sel, scroll_delta, max_line_idx);
                    }
                }
            }
            mouse_changed =
                app.viewport.scroll_top != prev_scroll || app.viewport.auto_follow_bottom != prev_follow;
        }
        MouseEventKind::ScrollDown => {
            let prev_scroll = app.viewport.scroll_top;
            let prev_follow = app.viewport.auto_follow_bottom;
            if app.viewport.scroll_inverted {
                app.viewport.scroll_top = app.viewport.scroll_top.saturating_sub(3);
            } else {
                app.viewport.scroll_top = (app.viewport.scroll_top.saturating_add(3)).min(max_scroll);
            }
            app.sync_auto_follow_bottom(max_scroll);
            let scroll_delta = app.viewport.scroll_top as isize - prev_scroll as isize;
            if scroll_delta != 0 && app.viewport.mouse_drag_mode != MouseDragMode::Scroll {
                let max_line_idx = app.rendered_line_count().saturating_sub(1);
                if let Some(sel) = app.viewport.selection.as_mut() {
                    if sel.dragging {
                        shift_selection_focus(sel, scroll_delta, max_line_idx);
                    }
                }
            }
            mouse_changed =
                app.viewport.scroll_top != prev_scroll || app.viewport.auto_follow_bottom != prev_follow;
        }
        MouseEventKind::Down(MouseButton::Middle)
            if m.modifiers.contains(KeyModifiers::CONTROL)
                && m.modifiers.contains(KeyModifiers::ALT) =>
        {
            app.viewport.mobile_plain_pending_coords = true;
            app.viewport.mobile_plain_suppress_coords = false;
            app.viewport.mobile_plain_new_gesture = true;
            app.viewport.mobile_mouse_buffer.clear();
        }
        MouseEventKind::Down(MouseButton::Left) => {
            app.viewport.mouse_drag_mode = MouseDragMode::Undecided;
            app.viewport.mouse_drag_last_row = clamped_y;
            if in_messages {
                app.viewport.selection = Some(Selection {
                    anchor_x: norm_x,
                    anchor_line_idx: clamped_line_idx,
                    focus_x: norm_x,
                    focus_line_idx: clamped_line_idx,
                    dragging: true,
                });
                mouse_changed = true;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(sel) = app.viewport.selection.as_mut() {
                if sel.dragging {
                    if app.viewport.mouse_drag_mode == MouseDragMode::Undecided {
                        app.viewport.mouse_drag_mode = decide_mouse_drag_mode(
                            sel.anchor_x,
                            app.viewport.mouse_drag_last_row,
                            norm_x,
                            clamped_y,
                        );
                    }

                    match app.viewport.mouse_drag_mode {
                        MouseDragMode::Scroll => {
                            let prev_scroll = app.viewport.scroll_top;
                            let prev_follow = app.viewport.auto_follow_bottom;
                            if clamped_y > app.viewport.mouse_drag_last_row {
                                app.viewport.scroll_top = app
                                    .viewport
                                    .scroll_top
                                    .saturating_sub(clamped_y - app.viewport.mouse_drag_last_row);
                            } else if clamped_y < app.viewport.mouse_drag_last_row {
                                app.viewport.scroll_top = app
                                    .viewport
                                    .scroll_top
                                    .saturating_add(app.viewport.mouse_drag_last_row - clamped_y);
                            }
                            app.viewport.scroll_top = app.viewport.scroll_top.min(max_scroll);
                            app.sync_auto_follow_bottom(max_scroll);
                            app.viewport.mouse_drag_last_row = clamped_y;
                            mouse_changed = app.viewport.scroll_top != prev_scroll
                                || app.viewport.auto_follow_bottom != prev_follow;
                        }
                        MouseDragMode::Select | MouseDragMode::Undecided => {
                            let prev_focus_x = sel.focus_x;
                            let prev_focus_idx = sel.focus_line_idx;
                            sel.focus_x = norm_x;
                            sel.focus_line_idx = clamped_line_idx;
                            mouse_changed =
                                sel.focus_x != prev_focus_x || sel.focus_line_idx != prev_focus_idx;
                        }
                    }
                }
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            let mut selection_to_copy = None;
            if let Some(sel) = app.viewport.selection.as_mut() {
                if sel.dragging {
                    let prev_focus_x = sel.focus_x;
                    let prev_focus_idx = sel.focus_line_idx;
                    sel.focus_x = norm_x;
                    sel.focus_line_idx = clamped_line_idx;
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
        }
        _ => {}
    }

    TerminalEventResult::Continue {
        needs_draw: mouse_changed,
    }
}

fn handle_paste_event(app: &mut AppState, pasted: String) -> TerminalEventResult {
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
