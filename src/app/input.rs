use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui_textarea::TextArea;
use serde_json::Value;

use super::mobile_mouse::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, parse_mobile_mouse_coords,
    parse_repeated_plain_mobile_pair, take_mobile_mouse_buffer, MobileMouseConsume,
};
use super::notifications::{
    animation_poll_timeout, animation_tick, handle_server_message_line, is_ctrl_char,
    is_key_press_like, is_perf_toggle_key, ServerRequestAction,
};
use super::render::{
    compute_input_layout, is_newline_enter, last_assistant_message, normalize_pasted_text,
    render_main_view,
};
use super::selection::{
    decide_mouse_drag_mode, normalize_selection_x, shift_selection_focus, MouseDragMode, Selection,
};
use super::state::ApprovalChoice;
use super::transcript_render::transcript_content_width;
use super::{persist_runtime_defaults, with_terminal, AppState, TerminalSize, MSG_TOP};
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};
use crate::event::{spawn_event_forwarders, UiEvent};
use crate::protocol::{
    params_turn_interrupt, params_turn_start, params_turn_steer, AppServerClient,
};

fn trace_terminal_event(ev: &Event) {
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

pub(super) fn make_input_area() -> TextArea<'static> {
    TextArea::default()
}

pub(super) fn is_mobile_mouse_key_candidate(
    app: &AppState,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if app.mobile_plain_pending_coords || app.mobile_plain_suppress_coords {
        return matches!(code, KeyCode::Char(ch) if ch.is_ascii_digit() || ch == ';');
    }

    if !app.mobile_mouse_buffer.is_empty() {
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

fn submit_turn_text(client: &AppServerClient, app: &mut AppState, text: String) {
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
    client: &AppServerClient,
    app: &mut AppState,
    choice: ApprovalChoice,
) {
    let Some(pending) = app.pending_approval.clone() else {
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
    if app.rewind_mode {
        app.rewind_selected_message_idx()
    } else {
        None
    }
}

fn ensure_transcript_layout(app: &mut AppState, size: TerminalSize) {
    let render_started = Instant::now();
    app.ensure_rendered_lines(transcript_content_width(size), hidden_user_message_idx(app));
    if let Some(perf) = app.perf.as_mut() {
        perf.transcript_render.push(render_started.elapsed());
    }
}

pub(super) fn run_conversation_tui(
    client: &AppServerClient,
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Result<()> {
    const MAX_UI_DRAIN_PER_CYCLE: usize = 4096;
    const SERVER_BUDGET_WITH_INPUT: usize = 8;
    const SERVER_BUDGET_IDLE: usize = 256;

    with_terminal(|terminal| {
        let ui_rx = spawn_event_forwarders(server_events_rx);
        let mut deferred_server_lines: VecDeque<String> = VecDeque::new();

        let mut needs_draw = true;
        let mut last_anim_tick = 0u128;

        loop {
            if let Some(perf) = app.perf.as_mut() {
                perf.loop_count = perf.loop_count.saturating_add(1);
            }

            let size = terminal.size()?;
            let size = TerminalSize {
                width: size.width as usize,
                height: size.height as usize,
            };

            let loop_now = Instant::now();
            let working = app.active_turn_id.is_some();
            if !working {
                if let Some(next_turn_text) = app.dequeue_turn_input(loop_now) {
                    submit_turn_text(client, app, next_turn_text);
                    needs_draw = true;
                    continue;
                }
            }
            let tick = if working { animation_tick() } else { 0 };
            if working {
                if tick != last_anim_tick {
                    needs_draw = true;
                }
            } else if last_anim_tick != 0 {
                needs_draw = true;
            }

            if needs_draw {
                ensure_transcript_layout(app, size);
                let draw_started = Instant::now();
                terminal.draw(|frame| {
                    render_main_view(frame, app);
                })?;
                if let Some(perf) = app.perf.as_mut() {
                    perf.record_draw(draw_started.elapsed());
                }
                needs_draw = false;
                last_anim_tick = tick;
            }

            let wait_started = Instant::now();
            let next_event = if !deferred_server_lines.is_empty() {
                match ui_rx.try_recv() {
                    Ok(ev) => Some(ev),
                    Err(TryRecvError::Empty) => None,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            } else if working {
                match ui_rx.recv_timeout(animation_poll_timeout()) {
                    Ok(ev) => Some(ev),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            } else if let Some(wait) = app.pending_ralph_continuation_wait(loop_now) {
                match ui_rx.recv_timeout(wait) {
                    Ok(ev) => Some(ev),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                }
            } else {
                match ui_rx.recv() {
                    Ok(ev) => Some(ev),
                    Err(_) => return Ok(()),
                }
            };
            if let Some(perf) = app.perf.as_mut() {
                perf.poll_wait.push(wait_started.elapsed());
            }

            let mut incoming_events = Vec::new();
            if let Some(ev) = next_event {
                incoming_events.push(ev);
            }
            for _ in 0..MAX_UI_DRAIN_PER_CYCLE {
                match ui_rx.try_recv() {
                    Ok(ev) => incoming_events.push(ev),
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
            let has_terminal_input = incoming_events
                .iter()
                .any(|ev| matches!(ev, UiEvent::Terminal(_)));
            let server_budget = if has_terminal_input {
                SERVER_BUDGET_WITH_INPUT
            } else {
                SERVER_BUDGET_IDLE
            };
            let prioritized_events =
                prioritize_events(incoming_events, &mut deferred_server_lines, server_budget);
            if prioritized_events.is_empty() {
                continue;
            }

            for next_event in prioritized_events {
                let event_started = Instant::now();
                match next_event {
                    UiEvent::ServerLine(line) => {
                        if let Some(perf) = app.perf.as_mut() {
                            perf.notifications = perf.notifications.saturating_add(1);
                        }
                        if let Some(action) = handle_server_message_line(app, &line) {
                            match action {
                                ServerRequestAction::ReplyError {
                                    request_id,
                                    code,
                                    message,
                                } => {
                                    if let Err(err) =
                                        client.respond_error(&request_id, code, &message)
                                    {
                                        app.set_status(format!(
                                            "server request reply failed: {err}"
                                        ));
                                    }
                                }
                            }
                        }
                        needs_draw = true;
                    }
                    UiEvent::Terminal(ev) => {
                        trace_terminal_event(&ev);
                        match ev {
                            Event::Key(k) => {
                                if let Some(perf) = app.perf.as_mut() {
                                    perf.mark_key_kind(k.kind);
                                }
                                if !is_key_press_like(k.kind) {
                                    continue;
                                }
                                if let Some(perf) = app.perf.as_mut() {
                                    perf.mark_key_event();
                                }
                                if is_perf_toggle_key(k.code, k.modifiers) {
                                    if let Some(perf) = app.perf.as_mut() {
                                        perf.toggle_overlay();
                                    }
                                    needs_draw = true;
                                    continue;
                                }
                                if matches!(k.code, KeyCode::F(6)) && k.modifiers.is_empty() {
                                    app.scroll_inverted = !app.scroll_inverted;
                                    needs_draw = true;
                                    continue;
                                }
                                let now = Instant::now();
                                app.expire_esc_chord(now);
                                if k.code != KeyCode::Esc {
                                    app.reset_esc_chord();
                                }
                                if app.show_help {
                                    match (k.code, k.modifiers) {
                                        (code, mods) if is_ctrl_char(code, mods, 'c') => {
                                            return Ok(())
                                        }
                                        (KeyCode::Esc, _) => {
                                            app.show_help = false;
                                        }
                                        (KeyCode::Char('?'), _) => {
                                            app.show_help = false;
                                        }
                                        _ => {}
                                    }
                                    needs_draw = true;
                                    continue;
                                }
                                if app.pending_approval.is_some() {
                                    match (k.code, k.modifiers) {
                                        (code, mods) if is_ctrl_char(code, mods, 'c') => {
                                            return Ok(())
                                        }
                                        (KeyCode::Char('y'), mods) if mods.is_empty() => {
                                            respond_to_pending_approval(
                                                client,
                                                app,
                                                ApprovalChoice::Accept,
                                            );
                                        }
                                        (KeyCode::Char('a'), mods) if mods.is_empty() => {
                                            respond_to_pending_approval(
                                                client,
                                                app,
                                                ApprovalChoice::Accept,
                                            );
                                        }
                                        (KeyCode::Char('s'), mods) if mods.is_empty() => {
                                            respond_to_pending_approval(
                                                client,
                                                app,
                                                ApprovalChoice::AcceptForSession,
                                            );
                                        }
                                        (KeyCode::Char('n'), mods) if mods.is_empty() => {
                                            respond_to_pending_approval(
                                                client,
                                                app,
                                                ApprovalChoice::Decline,
                                            );
                                        }
                                        (KeyCode::Char('c'), mods) if mods.is_empty() => {
                                            respond_to_pending_approval(
                                                client,
                                                app,
                                                ApprovalChoice::Cancel,
                                            );
                                        }
                                        _ => {}
                                    }
                                    needs_draw = true;
                                    continue;
                                }
                                if app.show_model_settings {
                                    match (k.code, k.modifiers) {
                                        (code, mods) if is_ctrl_char(code, mods, 'c') => {
                                            return Ok(())
                                        }
                                        (KeyCode::Esc, _) => app.close_model_settings(),
                                        (KeyCode::Tab, _) => app.model_settings_move_field(true),
                                        (KeyCode::BackTab, _) => {
                                            app.model_settings_move_field(false)
                                        }
                                        (KeyCode::Up, _) => app.model_settings_move_field(false),
                                        (KeyCode::Down, _) => app.model_settings_move_field(true),
                                        (KeyCode::Left, _) => match app.model_settings_field {
                                            super::state::ModelSettingsField::Model => {
                                                app.model_settings_cycle_model(-1);
                                            }
                                            super::state::ModelSettingsField::Effort => {
                                                app.model_settings_cycle_effort(-1);
                                            }
                                            super::state::ModelSettingsField::Summary => {
                                                app.model_settings_cycle_summary(-1);
                                            }
                                        },
                                        (KeyCode::Right, _) => match app.model_settings_field {
                                            super::state::ModelSettingsField::Model => {
                                                app.model_settings_cycle_model(1);
                                            }
                                            super::state::ModelSettingsField::Effort => {
                                                app.model_settings_cycle_effort(1);
                                            }
                                            super::state::ModelSettingsField::Summary => {
                                                app.model_settings_cycle_summary(1);
                                            }
                                        },
                                        (KeyCode::Backspace, _)
                                            if matches!(
                                                app.model_settings_field,
                                                super::state::ModelSettingsField::Model
                                            ) && !app.model_settings_has_model_choices() =>
                                        {
                                            app.model_settings_backspace();
                                        }
                                        (KeyCode::Enter, _) => {
                                            let defaults = app.apply_model_settings();
                                            if let Err(err) = persist_runtime_defaults(&defaults) {
                                                app.set_status(format!(
                                                    "saved for next turn; default save failed: {err}"
                                                ));
                                            }
                                        }
                                        (KeyCode::Char(ch), mods)
                                            if !mods.contains(KeyModifiers::CONTROL)
                                                && !mods.contains(KeyModifiers::ALT)
                                                && matches!(
                                                    app.model_settings_field,
                                                    super::state::ModelSettingsField::Model
                                                ) =>
                                        {
                                            if !app.model_settings_has_model_choices() {
                                                app.model_settings_insert_char(ch);
                                            }
                                        }
                                        _ => {}
                                    }
                                    needs_draw = true;
                                    continue;
                                }

                                if is_mobile_mouse_key_candidate(app, k.code, k.modifiers) {
                                    if let KeyCode::Char(ch) = k.code {
                                        let was_plain_capture = app.mobile_plain_pending_coords;
                                        match consume_mobile_mouse_char(app, ch) {
                                            MobileMouseConsume::PassThrough => {}
                                            MobileMouseConsume::Consumed => {
                                                needs_draw = true;
                                                continue;
                                            }
                                            MobileMouseConsume::Emit(text) => {
                                                if !text.is_empty() && !was_plain_capture {
                                                    app.input_insert_text(text);
                                                }
                                                needs_draw = true;
                                                continue;
                                            }
                                        }
                                    }
                                } else if let Some(text) = take_mobile_mouse_buffer(app) {
                                    if !text.is_empty()
                                        && !app.mobile_plain_pending_coords
                                        && !app.mobile_plain_suppress_coords
                                    {
                                        app.input_insert_text(text);
                                    }
                                    app.mobile_plain_pending_coords = false;
                                    app.mobile_plain_suppress_coords = false;
                                }

                                match (k.code, k.modifiers) {
                                    (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
                                    (KeyCode::Char('m'), KeyModifiers::CONTROL) => {
                                        app.toggle_model_settings();
                                    }
                                    (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                                        if let Err(e) = app.request_ralph_toggle() {
                                            app.set_status(format!("ralph: {e}"));
                                        }
                                    }
                                    (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                                        if let Some(sel) = app.selection {
                                            let copied = app.selected_text(sel);
                                            if !copied.is_empty() {
                                                let _ = try_copy_clipboard(&copied);
                                            }
                                        } else if let Some(text) =
                                            last_assistant_message(&app.messages)
                                        {
                                            let backend = try_copy_clipboard(text);
                                            app.set_status(format!(
                                                "copied last assistant message ({})",
                                                clipboard_backend_label(backend)
                                            ));
                                        }
                                    }
                                    (KeyCode::Char('l'), KeyModifiers::CONTROL) => {
                                        app.selection = None;
                                        app.mouse_drag_mode = MouseDragMode::Undecided;
                                        app.set_status("selection cleared");
                                    }
                                    (KeyCode::Esc, mods) if mods.is_empty() => {
                                        if app.rewind_mode {
                                            app.exit_rewind_mode_restore();
                                            app.reset_esc_chord();
                                        } else if let Some(turn_id) = app.active_turn_id.clone() {
                                            app.reset_esc_chord();
                                            let params =
                                                params_turn_interrupt(&app.thread_id, &turn_id);
                                            match client.call(
                                                "turn/interrupt",
                                                params,
                                                Duration::from_secs(10),
                                            ) {
                                                Ok(_) => {
                                                    app.append_turn_interrupted_marker();
                                                    app.set_status("interrupt requested");
                                                }
                                                Err(e) => app.set_status(format!("{e}")),
                                            }
                                        } else if app.register_escape_press(now) {
                                            app.selection = None;
                                            app.mouse_drag_mode = MouseDragMode::Undecided;
                                            if app.input_is_empty()
                                                && app.has_pending_ralph_continuation()
                                            {
                                                app.disable_ralph_mode();
                                            } else if app.input_is_empty() {
                                                app.enter_rewind_mode();
                                                app.align_rewind_scroll_to_selected_prompt(size);
                                            } else {
                                                app.clear_input();
                                            }
                                        } else {
                                            // First Esc press arms the chord; second Esc triggers action.
                                        }
                                    }
                                    (KeyCode::Home, _) if app.input_is_empty() => {
                                        ensure_transcript_layout(app, size);
                                        app.auto_follow_bottom = false;
                                        app.scroll_top = 0;
                                    }
                                    (KeyCode::End, _) if app.input_is_empty() => {
                                        ensure_transcript_layout(app, size);
                                        app.auto_follow_bottom = true;
                                    }
                                    (KeyCode::Up, _) => {
                                        if app.navigate_input_history_up() {
                                            app.align_rewind_scroll_to_selected_prompt(size);
                                        }
                                    }
                                    (KeyCode::Down, _) => {
                                        if app.navigate_input_history_down() {
                                            app.align_rewind_scroll_to_selected_prompt(size);
                                        }
                                    }
                                    (KeyCode::PageUp, _) => {
                                        ensure_transcript_layout(app, size);
                                        let msg_bottom = compute_input_layout(app, size).msg_bottom;
                                        let msg_height = if msg_bottom >= MSG_TOP {
                                            msg_bottom - MSG_TOP + 1
                                        } else {
                                            0
                                        };
                                        let max_scroll =
                                            app.rendered_line_count().saturating_sub(msg_height);
                                        app.scroll_top = app.scroll_top.saturating_sub(10);
                                        app.sync_auto_follow_bottom(max_scroll);
                                    }
                                    (KeyCode::PageDown, _) => {
                                        ensure_transcript_layout(app, size);
                                        let msg_bottom = compute_input_layout(app, size).msg_bottom;
                                        let msg_height = if msg_bottom >= MSG_TOP {
                                            msg_bottom - MSG_TOP + 1
                                        } else {
                                            0
                                        };
                                        let max_scroll =
                                            app.rendered_line_count().saturating_sub(msg_height);
                                        app.scroll_top =
                                            app.scroll_top.saturating_add(10).min(max_scroll);
                                        app.sync_auto_follow_bottom(max_scroll);
                                    }
                                    (KeyCode::Enter, mods) if is_newline_enter(mods) => {
                                        app.input_apply_key(k);
                                    }
                                    (KeyCode::Enter, _) => {
                                        if app.input_is_empty() {
                                            needs_draw = true;
                                            continue;
                                        }

                                        let rewind_target_idx = if app.rewind_mode {
                                            app.rewind_selected_message_idx()
                                        } else {
                                            None
                                        };
                                        let text = app.input_text();
                                        app.clear_rewind_mode_state();
                                        app.rewind_fork_from_message_idx(rewind_target_idx);
                                        app.push_input_history(&text);
                                        app.clear_input();
                                        app.selection = None;
                                        app.mark_user_turn_submitted();
                                        submit_turn_text(client, app, text);
                                    }
                                    (KeyCode::Char('?'), _) => {
                                        if app.input_is_empty() {
                                            app.show_help = true;
                                        } else {
                                            app.input_apply_key(k);
                                        }
                                    }
                                    _ => {
                                        app.input_apply_key(k);
                                    }
                                }
                                needs_draw = true;
                            }
                            Event::Mouse(m) => {
                                if let Some(perf) = app.perf.as_mut() {
                                    perf.mouse_events = perf.mouse_events.saturating_add(1);
                                }
                                if app.show_help || app.show_model_settings {
                                    continue;
                                }
                                ensure_transcript_layout(app, size);

                                let msg_top = MSG_TOP;
                                let msg_bottom = compute_input_layout(app, size).msg_bottom;
                                if msg_bottom < msg_top {
                                    continue;
                                }

                                let row1 = m.row as usize + 1;
                                let in_messages = row1 >= msg_top && row1 <= msg_bottom;
                                let norm_x = normalize_selection_x(m.column as usize);
                                let clamped_y = row1.clamp(msg_top, msg_bottom);
                                let clamped_line_idx = app.scroll_top + (clamped_y - msg_top);
                                let msg_height = msg_bottom - msg_top + 1;
                                let max_scroll =
                                    app.rendered_line_count().saturating_sub(msg_height);
                                let mut mouse_changed = false;

                                match m.kind {
                                    MouseEventKind::ScrollUp => {
                                        let prev_scroll = app.scroll_top;
                                        let prev_follow = app.auto_follow_bottom;
                                        if app.scroll_inverted {
                                            app.scroll_top =
                                                (app.scroll_top.saturating_add(3)).min(max_scroll);
                                        } else {
                                            app.scroll_top = app.scroll_top.saturating_sub(3);
                                        }
                                        app.sync_auto_follow_bottom(max_scroll);
                                        let scroll_delta =
                                            app.scroll_top as isize - prev_scroll as isize;
                                        if scroll_delta != 0
                                            && app.mouse_drag_mode != MouseDragMode::Scroll
                                        {
                                            let max_line_idx =
                                                app.rendered_line_count().saturating_sub(1);
                                            if let Some(sel) = app.selection.as_mut() {
                                                if sel.dragging {
                                                    shift_selection_focus(
                                                        sel,
                                                        scroll_delta,
                                                        max_line_idx,
                                                    );
                                                }
                                            }
                                        }
                                        mouse_changed = app.scroll_top != prev_scroll
                                            || app.auto_follow_bottom != prev_follow;
                                    }
                                    MouseEventKind::ScrollDown => {
                                        let prev_scroll = app.scroll_top;
                                        let prev_follow = app.auto_follow_bottom;
                                        if app.scroll_inverted {
                                            app.scroll_top = app.scroll_top.saturating_sub(3);
                                        } else {
                                            app.scroll_top =
                                                (app.scroll_top.saturating_add(3)).min(max_scroll);
                                        }
                                        app.sync_auto_follow_bottom(max_scroll);
                                        let scroll_delta =
                                            app.scroll_top as isize - prev_scroll as isize;
                                        if scroll_delta != 0
                                            && app.mouse_drag_mode != MouseDragMode::Scroll
                                        {
                                            let max_line_idx =
                                                app.rendered_line_count().saturating_sub(1);
                                            if let Some(sel) = app.selection.as_mut() {
                                                if sel.dragging {
                                                    shift_selection_focus(
                                                        sel,
                                                        scroll_delta,
                                                        max_line_idx,
                                                    );
                                                }
                                            }
                                        }
                                        mouse_changed = app.scroll_top != prev_scroll
                                            || app.auto_follow_bottom != prev_follow;
                                    }
                                    MouseEventKind::Down(MouseButton::Middle)
                                        if m.modifiers.contains(KeyModifiers::CONTROL)
                                            && m.modifiers.contains(KeyModifiers::ALT) =>
                                    {
                                        app.mobile_plain_pending_coords = true;
                                        app.mobile_plain_suppress_coords = false;
                                        app.mobile_plain_new_gesture = true;
                                        app.mobile_mouse_buffer.clear();
                                    }
                                    MouseEventKind::Down(MouseButton::Left) => {
                                        app.mouse_drag_mode = MouseDragMode::Undecided;
                                        app.mouse_drag_last_row = clamped_y;
                                        if in_messages {
                                            app.selection = Some(Selection {
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
                                        if let Some(sel) = app.selection.as_mut() {
                                            if sel.dragging {
                                                if app.mouse_drag_mode == MouseDragMode::Undecided {
                                                    app.mouse_drag_mode = decide_mouse_drag_mode(
                                                        sel.anchor_x,
                                                        app.mouse_drag_last_row,
                                                        norm_x,
                                                        clamped_y,
                                                    );
                                                }

                                                match app.mouse_drag_mode {
                                                    MouseDragMode::Scroll => {
                                                        let prev_scroll = app.scroll_top;
                                                        let prev_follow = app.auto_follow_bottom;
                                                        if clamped_y > app.mouse_drag_last_row {
                                                            app.scroll_top =
                                                                app.scroll_top.saturating_sub(
                                                                    clamped_y
                                                                        - app.mouse_drag_last_row,
                                                                );
                                                        } else if clamped_y
                                                            < app.mouse_drag_last_row
                                                        {
                                                            app.scroll_top =
                                                                app.scroll_top.saturating_add(
                                                                    app.mouse_drag_last_row
                                                                        - clamped_y,
                                                                );
                                                        }
                                                        app.scroll_top =
                                                            app.scroll_top.min(max_scroll);
                                                        app.sync_auto_follow_bottom(max_scroll);
                                                        app.mouse_drag_last_row = clamped_y;
                                                        mouse_changed = app.scroll_top
                                                            != prev_scroll
                                                            || app.auto_follow_bottom
                                                                != prev_follow;
                                                    }
                                                    MouseDragMode::Select
                                                    | MouseDragMode::Undecided => {
                                                        let prev_focus_x = sel.focus_x;
                                                        let prev_focus_idx = sel.focus_line_idx;
                                                        sel.focus_x = norm_x;
                                                        sel.focus_line_idx = clamped_line_idx;
                                                        mouse_changed = sel.focus_x != prev_focus_x
                                                            || sel.focus_line_idx != prev_focus_idx;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    MouseEventKind::Up(MouseButton::Left) => {
                                        let mut selection_to_copy = None;
                                        if let Some(sel) = app.selection.as_mut() {
                                            if sel.dragging {
                                                let prev_focus_x = sel.focus_x;
                                                let prev_focus_idx = sel.focus_line_idx;
                                                sel.focus_x = norm_x;
                                                sel.focus_line_idx = clamped_line_idx;
                                                sel.dragging = false;
                                                mouse_changed = sel.focus_x != prev_focus_x
                                                    || sel.focus_line_idx != prev_focus_idx
                                                    || !sel.dragging;

                                                if app.mouse_drag_mode == MouseDragMode::Scroll {
                                                    app.selection = None;
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
                                        app.mouse_drag_mode = MouseDragMode::Undecided;
                                    }
                                    _ => {}
                                }
                                if mouse_changed {
                                    needs_draw = true;
                                }
                            }
                            Event::Paste(pasted) => {
                                if let Some(perf) = app.perf.as_mut() {
                                    perf.paste_events = perf.paste_events.saturating_add(1);
                                }
                                if app.show_help {
                                    needs_draw = true;
                                    continue;
                                }
                                if app.show_model_settings {
                                    if matches!(
                                        app.model_settings_field,
                                        super::state::ModelSettingsField::Model
                                    ) && !app.model_settings_has_model_choices()
                                    {
                                        let normalized = normalize_pasted_text(&pasted);
                                        let first_line = normalized.lines().next().unwrap_or("");
                                        if !first_line.is_empty() {
                                            app.model_settings_model_input.push_str(first_line);
                                        }
                                    }
                                    needs_draw = true;
                                    continue;
                                }
                                if let Some((_, y)) = parse_repeated_plain_mobile_pair(&pasted) {
                                    apply_mobile_mouse_scroll(app, y);
                                    needs_draw = true;
                                    continue;
                                }
                                if app.input_is_empty() {
                                    if let Some((_, y)) = parse_mobile_mouse_coords(&pasted) {
                                        apply_mobile_mouse_scroll(app, y);
                                        needs_draw = true;
                                        continue;
                                    }
                                }
                                let normalized = normalize_pasted_text(&pasted);
                                if !normalized.is_empty() {
                                    app.input_insert_text(normalized);
                                    needs_draw = true;
                                }
                            }
                            Event::Resize(_, _) => {
                                if let Some(perf) = app.perf.as_mut() {
                                    perf.resize_events = perf.resize_events.saturating_add(1);
                                }
                                needs_draw = true;
                            }
                            _ => {}
                        }
                    }
                }
                if let Some(perf) = app.perf.as_mut() {
                    perf.event_handle.push(event_started.elapsed());
                }
            }

            if needs_draw {
                ensure_transcript_layout(app, size);
                let draw_started = Instant::now();
                terminal.draw(|frame| {
                    render_main_view(frame, app);
                })?;
                if let Some(perf) = app.perf.as_mut() {
                    perf.record_draw(draw_started.elapsed());
                }
                needs_draw = false;
                last_anim_tick = if app.active_turn_id.is_some() {
                    animation_tick()
                } else {
                    0
                };
            }
        }
    })
}

pub(super) fn prioritize_events(
    incoming_events: Vec<UiEvent>,
    deferred_server_lines: &mut VecDeque<String>,
    server_budget: usize,
) -> Vec<UiEvent> {
    let mut prioritized = Vec::with_capacity(incoming_events.len());
    let mut priority_server_lines: Vec<String> = Vec::new();
    for event in incoming_events {
        match event {
            UiEvent::Terminal(ev) => prioritized.push(UiEvent::Terminal(ev)),
            UiEvent::ServerLine(line) => {
                if is_priority_server_line(&line) {
                    priority_server_lines.push(line);
                } else {
                    deferred_server_lines.push_back(line);
                }
            }
        }
    }
    for line in priority_server_lines {
        prioritized.push(UiEvent::ServerLine(line));
    }

    while let Some(idx) = deferred_server_lines
        .iter()
        .position(|line| is_priority_server_line(line))
    {
        if let Some(line) = deferred_server_lines.remove(idx) {
            prioritized.push(UiEvent::ServerLine(line));
        } else {
            break;
        }
    }

    for _ in 0..server_budget {
        let Some(line) = deferred_server_lines.pop_front() else {
            break;
        };
        prioritized.push(UiEvent::ServerLine(line));
    }
    prioritized
}

pub(super) fn is_priority_server_line(line: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return false;
    };
    matches!(method, "turn/completed" | "turn/started" | "error")
}
