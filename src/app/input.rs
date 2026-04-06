use std::collections::VecDeque;
use std::sync::mpsc::{RecvTimeoutError, TryRecvError};
use std::time::Instant;

use anyhow::Result;
use serde_json::Value;

use super::input_events::{
    ensure_transcript_layout, handle_terminal_event, submit_turn_text_with_history,
    TerminalEventResult,
};
#[cfg(test)]
pub(super) use super::input_events::is_mobile_mouse_key_candidate;
use super::notifications::{
    animation_poll_timeout, animation_tick, handle_server_message_line, ServerRequestAction,
};
use super::render::render_main_view;
use super::{with_terminal, AppState, TerminalSize};
use crate::backend::BackendClient;
use crate::event::{spawn_event_forwarders, UiEvent};

pub(super) fn make_input_area() -> ratatui_textarea::TextArea<'static> {
    ratatui_textarea::TextArea::default()
}

pub(super) fn run_conversation_tui(
    client: &dyn BackendClient,
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
            let mut prefetched_event = None;
            if !working && deferred_server_lines.is_empty() {
                match ui_rx.try_recv() {
                    Ok(ev) => prefetched_event = Some(ev),
                    Err(TryRecvError::Empty) => {}
                    Err(TryRecvError::Disconnected) => return Ok(()),
                }
            }
            if can_submit_queued_turn(working, &deferred_server_lines, prefetched_event.as_ref()) {
                if let Some(next_turn) = app.dequeue_turn_input(loop_now) {
                    submit_turn_text_with_history(
                        client,
                        app,
                        next_turn.text,
                        next_turn.record_input_history,
                    );
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
            let next_event = if let Some(ev) = prefetched_event {
                Some(ev)
            } else if !deferred_server_lines.is_empty() {
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
                        match handle_terminal_event(client, app, ev, size) {
                            TerminalEventResult::Quit => return Ok(()),
                            TerminalEventResult::Continue {
                                needs_draw: event_needs_draw,
                            } => {
                                if event_needs_draw {
                                    needs_draw = true;
                                }
                            }
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

pub(super) fn can_submit_queued_turn(
    working: bool,
    deferred_server_lines: &VecDeque<String>,
    prefetched_event: Option<&UiEvent>,
) -> bool {
    !working && deferred_server_lines.is_empty() && prefetched_event.is_none()
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
