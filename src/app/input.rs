use std::collections::VecDeque;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers, MouseButton, MouseEventKind};
use ratatui_textarea::TextArea;
use serde_json::Value;

use super::mobile_mouse::{
    apply_mobile_mouse_scroll, consume_mobile_mouse_char, parse_mobile_mouse_coords,
    take_mobile_mouse_buffer, MobileMouseConsume,
};
use super::notifications::{
    animation_poll_timeout, animation_tick, handle_notification_line, is_ctrl_char,
    is_key_press_like, is_perf_toggle_key,
};
use super::render::{
    compute_input_layout, is_newline_enter, last_assistant_message, normalize_pasted_text,
    render_main_view, transcript_content_width,
};
use super::selection::{
    decide_mouse_drag_mode, normalize_selection_x, selected_text, MouseDragMode, Selection,
};
use super::{with_terminal, AppState, TerminalSize, MSG_TOP};
use crate::clipboard::{clipboard_backend_label, try_copy_clipboard};
use crate::event::{spawn_event_forwarders, UiEvent};
use crate::protocol::{
    params_turn_interrupt, params_turn_start, params_turn_steer, AppServerClient,
};

pub(super) fn make_input_area() -> TextArea<'static> {
    TextArea::default()
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
        let params = params_turn_start(&app.thread_id, &text);
        match client.call("turn/start", params, Duration::from_secs(10)) {
            Ok(_) => app.set_status("sent turn"),
            Err(e) => app.set_status(format!("{e}")),
        }
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
            let render_started = Instant::now();
            let hidden_user_message_idx = if app.rewind_mode {
                app.rewind_selected_message_idx()
            } else {
                None
            };
            app.ensure_rendered_lines(transcript_content_width(size), hidden_user_message_idx);
            if let Some(perf) = app.perf.as_mut() {
                perf.transcript_render.push(render_started.elapsed());
            }

            let working = app.active_turn_id.is_some();
            if !working {
                if let Some(next_turn_text) = app.dequeue_turn_input() {
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
                match ui_rx.recv_timeout(animation_poll_timeout(true)) {
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
                        handle_notification_line(app, &line);
                        needs_draw = true;
                    }
                    UiEvent::Terminal(ev) => match ev {
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
                                    (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
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

                            if k.modifiers.is_empty() {
                                if let KeyCode::Char(ch) = k.code {
                                    match consume_mobile_mouse_char(app, ch) {
                                        MobileMouseConsume::PassThrough => {}
                                        MobileMouseConsume::Consumed => {
                                            needs_draw = true;
                                            continue;
                                        }
                                        MobileMouseConsume::Emit(text) => {
                                            if !text.is_empty() {
                                                app.input_insert_text(text);
                                            }
                                            needs_draw = true;
                                            continue;
                                        }
                                    }
                                } else if let Some(text) = take_mobile_mouse_buffer(app) {
                                    if !text.is_empty() {
                                        app.input_insert_text(text);
                                    }
                                }
                            } else if let Some(text) = take_mobile_mouse_buffer(app) {
                                if !text.is_empty() {
                                    app.input_insert_text(text);
                                }
                            }

                            match (k.code, k.modifiers) {
                                (code, mods) if is_ctrl_char(code, mods, 'c') => return Ok(()),
                                (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
                                    if let Err(e) = app.request_ralph_toggle() {
                                        app.set_status(format!("ralph: {e}"));
                                    }
                                }
                                (KeyCode::Char('y'), KeyModifiers::CONTROL) => {
                                    if let Some(sel) = app.selection {
                                        let msg_bottom = compute_input_layout(app, size).msg_bottom;
                                        let copied = selected_text(
                                            sel,
                                            &app.rendered_lines,
                                            msg_bottom,
                                            app.scroll_top,
                                        );
                                        if !copied.is_empty() {
                                            let _ = try_copy_clipboard(&copied);
                                        }
                                    } else if let Some(text) = last_assistant_message(&app.messages)
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
                                        if app.input_is_empty() {
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
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = 0;
                                }
                                (KeyCode::End, _) if app.input_is_empty() => {
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
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = app.scroll_top.saturating_sub(10);
                                }
                                (KeyCode::PageDown, _) => {
                                    app.auto_follow_bottom = false;
                                    app.scroll_top = app.scroll_top.saturating_add(10);
                                }
                                (KeyCode::Enter, mods) if is_newline_enter(mods) => {
                                    app.input_apply_key(k);
                                }
                                (KeyCode::Enter, _) => {
                                    if app.input_is_empty() {
                                        needs_draw = true;
                                        continue;
                                    }

                                    let text = app.input_text();
                                    app.clear_rewind_mode_state();
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
                            if app.show_help {
                                continue;
                            }

                            let msg_top = MSG_TOP;
                            let msg_bottom = compute_input_layout(app, size).msg_bottom;
                            if msg_bottom < msg_top {
                                continue;
                            }

                            let row1 = m.row as usize + 1;
                            let in_messages = row1 >= msg_top && row1 <= msg_bottom;
                            let norm_x = normalize_selection_x(m.column as usize);
                            let clamped_y = row1.clamp(msg_top, msg_bottom);
                            let mut mouse_changed = false;

                            match m.kind {
                                MouseEventKind::ScrollUp => {
                                    let prev_scroll = app.scroll_top;
                                    let prev_follow = app.auto_follow_bottom;
                                    app.auto_follow_bottom = false;
                                    if app.scroll_inverted {
                                        app.scroll_top = app.scroll_top.saturating_add(3);
                                    } else {
                                        app.scroll_top = app.scroll_top.saturating_sub(3);
                                    }
                                    mouse_changed = app.scroll_top != prev_scroll
                                        || app.auto_follow_bottom != prev_follow;
                                }
                                MouseEventKind::ScrollDown => {
                                    let prev_scroll = app.scroll_top;
                                    let prev_follow = app.auto_follow_bottom;
                                    app.auto_follow_bottom = false;
                                    if app.scroll_inverted {
                                        app.scroll_top = app.scroll_top.saturating_sub(3);
                                    } else {
                                        app.scroll_top = app.scroll_top.saturating_add(3);
                                    }
                                    mouse_changed = app.scroll_top != prev_scroll
                                        || app.auto_follow_bottom != prev_follow;
                                }
                                MouseEventKind::Down(MouseButton::Left) => {
                                    app.mouse_drag_mode = MouseDragMode::Undecided;
                                    app.mouse_drag_last_row = clamped_y;
                                    if in_messages {
                                        app.selection = Some(Selection {
                                            anchor_x: norm_x,
                                            anchor_y: row1,
                                            focus_x: norm_x,
                                            focus_y: row1,
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
                                                    sel.anchor_y,
                                                    norm_x,
                                                    clamped_y,
                                                );
                                            }

                                            match app.mouse_drag_mode {
                                                MouseDragMode::Scroll => {
                                                    let prev_scroll = app.scroll_top;
                                                    let prev_follow = app.auto_follow_bottom;
                                                    app.auto_follow_bottom = false;
                                                    if clamped_y > app.mouse_drag_last_row {
                                                        app.scroll_top =
                                                            app.scroll_top.saturating_sub(
                                                                clamped_y - app.mouse_drag_last_row,
                                                            );
                                                    } else if clamped_y < app.mouse_drag_last_row {
                                                        app.scroll_top =
                                                            app.scroll_top.saturating_add(
                                                                app.mouse_drag_last_row - clamped_y,
                                                            );
                                                    }
                                                    app.mouse_drag_last_row = clamped_y;
                                                    mouse_changed = app.scroll_top != prev_scroll
                                                        || app.auto_follow_bottom != prev_follow;
                                                }
                                                MouseDragMode::Select
                                                | MouseDragMode::Undecided => {
                                                    let prev_focus_x = sel.focus_x;
                                                    let prev_focus_y = sel.focus_y;
                                                    sel.focus_x = norm_x;
                                                    sel.focus_y = clamped_y;
                                                    mouse_changed = sel.focus_x != prev_focus_x
                                                        || sel.focus_y != prev_focus_y;
                                                }
                                            }
                                        }
                                    }
                                }
                                MouseEventKind::Up(MouseButton::Left) => {
                                    if let Some(sel) = app.selection.as_mut() {
                                        if sel.dragging {
                                            let prev_focus_x = sel.focus_x;
                                            let prev_focus_y = sel.focus_y;
                                            sel.focus_x = norm_x;
                                            sel.focus_y = clamped_y;
                                            sel.dragging = false;
                                            mouse_changed = sel.focus_x != prev_focus_x
                                                || sel.focus_y != prev_focus_y
                                                || !sel.dragging;

                                            if app.mouse_drag_mode == MouseDragMode::Scroll {
                                                app.selection = None;
                                            } else {
                                                let copied = selected_text(
                                                    *sel,
                                                    &app.rendered_lines,
                                                    msg_bottom,
                                                    app.scroll_top,
                                                );
                                                if !copied.is_empty() {
                                                    let _ = try_copy_clipboard(&copied);
                                                }
                                            }
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
                    },
                }
                if let Some(perf) = app.perf.as_mut() {
                    perf.event_handle.push(event_started.elapsed());
                }
            }

            if needs_draw {
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
