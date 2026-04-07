//! Terminal event dispatch: keyboard event routing and resize handling.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crossterm::event::{Event, KeyCode, KeyModifiers};

use super::mobile_mouse::{consume_mobile_mouse_char, take_mobile_mouse_buffer, MobileMouseConsume};
use super::mouse_events::{handle_mouse_event, handle_paste_event};
use super::notifications::{is_ctrl_char, is_key_press_like, is_perf_toggle_key};
use super::render::{compute_input_layout, is_newline_enter, last_assistant_message};
use super::selection::MouseDragMode;
use super::state::{AppState, ApprovalChoice, ModelSettingsField};
use super::transcript_render::transcript_content_width;
use super::turn_submit::{interrupt_active_turn, respond_to_pending_approval, submit_turn_text};
use super::{persist_runtime_defaults, TerminalSize, MSG_TOP};
use crate::backend::BackendClient;
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};

// --- Types ---

pub(super) enum TerminalEventResult {
    Quit,
    Continue { needs_draw: bool },
}

// --- Event tracing ---

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

// --- Mobile mouse detection ---

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

// --- Transcript layout ---

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

// --- Event dispatch ---

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

// --- Key event handling ---

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
    if let Some(result) = handle_global_toggle_keys(app, k) {
        return result;
    }
    let now = Instant::now();
    app.expire_esc_chord(now);
    if k.code != KeyCode::Esc {
        app.reset_esc_chord();
    }
    if app.viewport.show_help {
        return handle_help_key(app, k);
    }
    if app.approval.pending.is_some() {
        return handle_approval_key(client, app, k);
    }
    if app.runtime.show_model_settings {
        return handle_model_settings_key(app, k);
    }
    if let Some(result) = handle_mobile_mouse_keys(app, k) {
        return result;
    }
    handle_normal_key(client, app, k, size, now)
}

fn handle_global_toggle_keys(
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
) -> Option<TerminalEventResult> {
    if is_perf_toggle_key(k.code, k.modifiers) {
        if let Some(perf) = app.perf.as_mut() {
            perf.toggle_overlay();
        }
        return Some(TerminalEventResult::Continue { needs_draw: true });
    }
    if matches!(k.code, KeyCode::F(6)) && k.modifiers.is_empty() {
        app.viewport.scroll_inverted = !app.viewport.scroll_inverted;
        return Some(TerminalEventResult::Continue { needs_draw: true });
    }
    None
}

fn handle_help_key(app: &mut AppState, k: crossterm::event::KeyEvent) -> TerminalEventResult {
    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => return TerminalEventResult::Quit,
        (KeyCode::Esc, _) | (KeyCode::Char('?'), _) => {
            app.viewport.show_help = false;
        }
        _ => {}
    }
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_approval_key(
    client: &dyn BackendClient,
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
) -> TerminalEventResult {
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
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_model_settings_key(
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
) -> TerminalEventResult {
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
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_mobile_mouse_keys(
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
) -> Option<TerminalEventResult> {
    if is_mobile_mouse_key_candidate(app, k.code, k.modifiers) {
        if let KeyCode::Char(ch) = k.code {
            let was_plain_capture = app.viewport.mobile_plain_pending_coords;
            match consume_mobile_mouse_char(app, ch) {
                MobileMouseConsume::PassThrough => {}
                MobileMouseConsume::Consumed => {
                    return Some(TerminalEventResult::Continue { needs_draw: true });
                }
                MobileMouseConsume::Emit(text) => {
                    if !text.is_empty() && !was_plain_capture {
                        app.input_insert_text(text);
                    }
                    return Some(TerminalEventResult::Continue { needs_draw: true });
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
    None
}

fn handle_normal_key(
    client: &dyn BackendClient,
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
    size: TerminalSize,
    now: Instant,
) -> TerminalEventResult {
    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => TerminalEventResult::Quit,
        (KeyCode::Char('m'), KeyModifiers::CONTROL) => {
            app.toggle_model_settings();
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => handle_ctrl_r_ralph(app),
        (KeyCode::Char('y'), KeyModifiers::CONTROL) => handle_ctrl_y_copy(app),
        (KeyCode::Char('l'), KeyModifiers::CONTROL) => handle_ctrl_l_clear_selection(app),
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
        (KeyCode::Up, _) => handle_history_navigate(app, size, true),
        (KeyCode::Down, _) => handle_history_navigate(app, size, false),
        (KeyCode::PageUp, _) => handle_page_scroll(app, size, false),
        (KeyCode::PageDown, _) => handle_page_scroll(app, size, true),
        (KeyCode::Enter, mods) if is_newline_enter(mods) => {
            app.input_apply_key(k);
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Enter, _) => handle_enter_submit(client, app),
        (KeyCode::Char('?'), _) if app.input_is_empty() => {
            app.viewport.show_help = true;
            TerminalEventResult::Continue { needs_draw: true }
        }
        _ => {
            app.input_apply_key(k);
            TerminalEventResult::Continue { needs_draw: true }
        }
    }
}

fn handle_ctrl_r_ralph(app: &mut AppState) -> TerminalEventResult {
    if let Err(e) = app.request_ralph_toggle() {
        app.set_status(format!("ralph: {e}"));
    }
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_ctrl_l_clear_selection(app: &mut AppState) -> TerminalEventResult {
    app.viewport.selection = None;
    app.viewport.mouse_drag_mode = MouseDragMode::Undecided;
    app.set_status("selection cleared");
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_history_navigate(
    app: &mut AppState,
    size: TerminalSize,
    up: bool,
) -> TerminalEventResult {
    let moved = if up {
        app.navigate_input_history_up()
    } else {
        app.navigate_input_history_down()
    };
    if moved {
        app.align_rewind_scroll_to_selected_prompt(size);
    }
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_ctrl_y_copy(app: &mut AppState) -> TerminalEventResult {
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

fn handle_page_scroll(app: &mut AppState, size: TerminalSize, down: bool) -> TerminalEventResult {
    ensure_transcript_layout(app, size);
    let msg_bottom = compute_input_layout(app, size).msg_bottom;
    let msg_height = if msg_bottom >= MSG_TOP {
        msg_bottom - MSG_TOP + 1
    } else {
        0
    };
    let max_scroll = app.rendered_line_count().saturating_sub(msg_height);
    if down {
        app.viewport.scroll_top = app.viewport.scroll_top.saturating_add(10).min(max_scroll);
    } else {
        app.viewport.scroll_top = app.viewport.scroll_top.saturating_sub(10);
    }
    app.sync_auto_follow_bottom(max_scroll);
    TerminalEventResult::Continue { needs_draw: true }
}

fn handle_enter_submit(
    client: &dyn BackendClient,
    app: &mut AppState,
) -> TerminalEventResult {
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
    if app.active_turn_id.is_some() {
        app.reset_esc_chord();
        interrupt_active_turn(client, app);
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
