//! Keyboard event routing: global toggles, overlay handlers, and normal input dispatch.
#[path = "input_events_dictation.rs"]
mod input_events_dictation;
#[path = "input_events_model_settings.rs"]
mod input_events_model_settings;
#[path = "input_events_submit.rs"]
mod input_events_submit;

use self::input_events_model_settings::handle_model_settings_key;
use self::input_events_submit::handle_enter_submit;

use std::time::Instant;

use crossterm::event::{KeyCode, KeyModifiers};

use super::{ensure_transcript_layout, is_mobile_mouse_key_candidate, TerminalEventResult};
use crate::app::mobile_mouse::{
    consume_mobile_mouse_char, take_mobile_mouse_buffer, MobileMouseConsume,
};
use crate::app::notifications::{is_ctrl_char, is_key_press_like, is_perf_toggle_key};
use crate::app::render::{compute_input_layout, is_newline_enter};
use crate::app::render_input::last_assistant_message;
use crate::app::selection::MouseDragMode;
use crate::app::state::{AppState, ApprovalChoice};
use crate::app::turn_submit::{interrupt_active_turn, respond_to_pending_approval};
use crate::app::{TerminalSize, MSG_TOP};
use crate::backend::BackendClient;
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};

use self::input_events_dictation::{
    handle_ctrl_d_dictation, handle_enter_stop_dictation, handle_f7_dictation_profile,
    handle_f9_dictation_endpoint,
};

pub(super) fn handle_key_event(
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
    if app.codex.enabled && k.code == KeyCode::F(5) && k.modifiers.is_empty() {
        crate::app::codex_reconnect::start(client, app);
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if let Some(result) = handle_global_toggle_keys(app, k) {
        return result;
    }
    if let Some(result) = crate::app::codex_panel_view::key(client, app, k) {
        return result;
    }
    if let Some(result) = crate::app::claude_task_view::key(client, app, k) {
        return result;
    }
    if app.codex.enabled && k.code == KeyCode::F(4) {
        app.codex.goals.show = false;
        crate::app::codex_callbacks::next(app);
        return TerminalEventResult::Continue { needs_draw: true };
    }
    let now = Instant::now();
    app.expire_esc_chord(now);
    if k.code != KeyCode::Esc {
        app.reset_esc_chord();
    }
    if app.viewport.show_help {
        return handle_help_key(app, k, size);
    }
    if app.runtime.show_model_settings && crate::app::native_questions::draft(app).is_some() {
        return handle_model_settings_key(client, app, k);
    }
    if crate::app::native_questions::modal(app) {
        if crate::app::native_questions::draft(app).is_some() {
            return super::super::claude_question_input::handle_key(client, app, k, size);
        }
        return handle_approval_key(client, app, k);
    }
    if app.runtime.show_model_settings {
        return handle_model_settings_key(client, app, k);
    }
    if let Some(result) = crate::app::codex_goal_view::key(client, app, k, size) {
        return result;
    }
    if let Some(result) = crate::app::codex_queue_view::key(client, app, k, size) {
        return result;
    }
    if let Some(result) = handle_claude_session_keys(client, app, k, size) {
        return result;
    }
    if let Some(result) = handle_mobile_mouse_keys(app, k) {
        return result;
    }
    handle_normal_key(client, app, k, size, now)
}

fn handle_claude_session_keys(
    client: &dyn BackendClient,
    app: &mut AppState,
    key: crossterm::event::KeyEvent,
    size: TerminalSize,
) -> Option<TerminalEventResult> {
    if client.kind() != crate::backend::BackendKind::Claude || !key.modifiers.is_empty() {
        return None;
    }
    match key.code {
        KeyCode::F(3) => {
            app.claude.prompts.queue = !app.claude.prompts.queue;
            super::super::input_mode::changed(app, app.claude.prompts.queue, Instant::now());
        }
        KeyCode::F(4) if app.rewind_mode() => {
            crate::app::claude_rewind::toggle_unknown(app);
            app.align_rewind_scroll_to_selected_prompt(size);
        }
        KeyCode::F(5)
            if app.claude.phase == crate::app::claude_state::ClaudePhase::Closed
                || crate::app::claude_session_history::has_unsent(app) =>
        {
            crate::app::claude_session_history::load_unsent(app);
        }
        _ => return None,
    }
    Some(TerminalEventResult::Continue { needs_draw: true })
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

fn handle_help_key(
    app: &mut AppState,
    k: crossterm::event::KeyEvent,
    size: TerminalSize,
) -> TerminalEventResult {
    let maximum = crate::app::overlay_render::help_scroll_limit(size);
    app.viewport.help_scroll = app.viewport.help_scroll.min(maximum);
    match (k.code, k.modifiers) {
        (code, mods) if is_ctrl_char(code, mods, 'c') => return TerminalEventResult::Quit,
        (KeyCode::Esc, _) | (KeyCode::Char('?'), _) => {
            app.viewport.show_help = false;
        }
        (KeyCode::Down, _) => {
            app.viewport.help_scroll = app.viewport.help_scroll.saturating_add(1).min(maximum)
        }
        (KeyCode::Up, _) => app.viewport.help_scroll = app.viewport.help_scroll.saturating_sub(1),
        (KeyCode::PageDown, _) => {
            app.viewport.help_scroll = app.viewport.help_scroll.saturating_add(5).min(maximum)
        }
        (KeyCode::PageUp, _) => {
            app.viewport.help_scroll = app.viewport.help_scroll.saturating_sub(5)
        }
        (KeyCode::Home, _) => app.viewport.help_scroll = 0,
        (KeyCode::End, _) => app.viewport.help_scroll = maximum,
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
        (code, mods) if is_ctrl_char(code, mods, 'd') => handle_ctrl_d_dictation(app),
        (KeyCode::F(7), mods) if mods.is_empty() => handle_f7_dictation_profile(app),
        (KeyCode::F(9), mods) if mods.is_empty() => handle_f9_dictation_endpoint(app),
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
        (KeyCode::Enter, mods) if mods.is_empty() && app.dictation_recording() => {
            handle_enter_stop_dictation(app)
        }
        (KeyCode::Enter, mods) if is_newline_enter(mods) => {
            app.input_apply_key(k);
            TerminalEventResult::Continue { needs_draw: true }
        }
        (KeyCode::Enter, _) => handle_enter_submit(client, app),
        (KeyCode::Char('?'), _) if app.input_is_empty() => {
            app.viewport.show_help = true;
            app.viewport.help_scroll = 0;
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

fn handle_escape_key(
    client: &dyn BackendClient,
    app: &mut AppState,
    size: TerminalSize,
    now: Instant,
) -> TerminalEventResult {
    if app.dictation_active() {
        app.cancel_dictation();
        app.reset_esc_chord();
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.ralph_enabled() {
        app.reset_esc_chord();
        app.disable_ralph_mode();
        if app.active_turn_id.is_some() {
            interrupt_active_turn(client, app);
        }
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.rewind_mode() {
        app.exit_rewind_mode_restore();
        app.reset_esc_chord();
        return TerminalEventResult::Continue { needs_draw: true };
    }
    if app.active_turn_id.is_some()
        || app.claude.activity.state == crate::app::claude_activity::NativeState::Running
    {
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
