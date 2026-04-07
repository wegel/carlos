use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Instant;

use anyhow::Result;
use crossterm::event::{Event as CrosstermEvent, MouseEventKind};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use serde_json::Value;

#[cfg(test)]
pub(super) use super::input_events::is_mobile_mouse_key_candidate;
use super::input_events::{ensure_transcript_layout, handle_terminal_event, TerminalEventResult};
use super::turn_submit::submit_turn_text_with_history;
use super::notifications::{
    animation_poll_timeout, animation_tick, handle_server_message_line, ServerRequestAction,
};
use super::render::render_main_view;
use super::{with_terminal, AppState, TerminalSize};
use crate::backend::BackendClient;
use crate::event::{spawn_event_forwarders, UiEvent};

const MAX_UI_DRAIN_PER_CYCLE: usize = 4096;
const SERVER_BUDGET_WITH_INPUT: usize = 8;
const SERVER_BUDGET_IDLE: usize = 256;

pub(super) fn make_input_area() -> ratatui_textarea::TextArea<'static> {
    ratatui_textarea::TextArea::default()
}

/// Result of draining or polling that may detect a channel disconnect.
enum ChannelOk<T> {
    Ok(T),
    Disconnected,
}

impl ChannelOk<()> {
    fn disconnected(&self) -> bool {
        matches!(self, ChannelOk::Disconnected)
    }
}

pub(super) fn run_conversation_tui(
    client: &dyn BackendClient,
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Result<()> {
    with_terminal(|terminal| {
        let ui_rx = spawn_event_forwarders(server_events_rx);
        let mut deferred = VecDeque::<String>::new();
        let mut prefetched = VecDeque::<UiEvent>::new();
        let mut needs_draw = true;
        let mut last_anim_tick = 0u128;
        loop {
            if let Some(p) = app.perf.as_mut() { p.loop_count = p.loop_count.saturating_add(1); }
            let size = terminal_size(terminal)?;
            let (loop_now, working) = (Instant::now(), app.active_turn_id.is_some());
            if drain_prefetched_idle(&ui_rx, working, &deferred, &mut prefetched).disconnected() {
                return Ok(());
            }
            if try_submit_queued_turn(client, app, working, &deferred, &prefetched, loop_now) {
                needs_draw = true;
                continue;
            }
            last_anim_tick = update_animation_state(working, last_anim_tick, &mut needs_draw);
            let tick = if working { animation_tick() } else { 0 };
            if needs_draw {
                draw_frame(terminal, app, size, tick, &mut needs_draw, &mut last_anim_tick)?;
            }
            let next_event = match poll_next_event(&ui_rx, &mut prefetched, &deferred, working, app, loop_now) {
                ChannelOk::Disconnected => return Ok(()),
                ChannelOk::Ok(ev) => ev,
            };
            if let Some(p) = app.perf.as_mut() { p.poll_wait.push(loop_now.elapsed()); }
            let incoming = match gather_incoming_events(next_event, &mut prefetched, &ui_rx) {
                ChannelOk::Disconnected => return Ok(()),
                ChannelOk::Ok(v) => v,
            };
            let events = budgeted_events(incoming, &mut deferred);
            if events.is_empty() { continue; }
            for ev in &events {
                if process_event(ev, client, app, size, &mut needs_draw) { return Ok(()); }
            }
            if needs_draw {
                let t = if app.active_turn_id.is_some() { animation_tick() } else { 0 };
                draw_frame(terminal, app, size, t, &mut needs_draw, &mut last_anim_tick)?;
            }
        }
    })
}

fn terminal_size(
    terminal: &Terminal<CrosstermBackend<std::io::Stdout>>,
) -> Result<TerminalSize> {
    let size = terminal.size()?;
    Ok(TerminalSize {
        width: size.width as usize,
        height: size.height as usize,
    })
}

/// When idle (not working and no deferred server lines), eagerly pull
/// events from the channel into the prefetch buffer.
fn drain_prefetched_idle(
    ui_rx: &Receiver<UiEvent>,
    working: bool,
    deferred_server_lines: &VecDeque<String>,
    prefetched_events: &mut VecDeque<UiEvent>,
) -> ChannelOk<()> {
    if working || !deferred_server_lines.is_empty() {
        return ChannelOk::Ok(());
    }
    while prefetched_events.len() < MAX_UI_DRAIN_PER_CYCLE {
        match ui_rx.try_recv() {
            Ok(ev) => prefetched_events.push_back(ev),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => return ChannelOk::Disconnected,
        }
    }
    ChannelOk::Ok(())
}

/// Attempt to submit a queued turn if conditions allow. Returns true
/// if a turn was submitted (caller should `continue` the loop).
fn try_submit_queued_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
    working: bool,
    deferred_server_lines: &VecDeque<String>,
    prefetched_events: &VecDeque<UiEvent>,
    now: Instant,
) -> bool {
    if !can_submit_queued_turn(working, deferred_server_lines, prefetched_events) {
        return false;
    }
    if let Some(next_turn) = app.dequeue_turn_input(now) {
        submit_turn_text_with_history(
            client,
            app,
            next_turn.text,
            next_turn.record_input_history,
        );
        return true;
    }
    false
}

/// Check animation state and return the current tick. Sets `needs_draw`
/// when the animation frame has changed.
fn update_animation_state(working: bool, last_anim_tick: u128, needs_draw: &mut bool) -> u128 {
    let tick = if working { animation_tick() } else { 0 };
    if working {
        if tick != last_anim_tick {
            *needs_draw = true;
        }
    } else if last_anim_tick != 0 {
        *needs_draw = true;
    }
    // Return the old tick; actual update happens in draw_frame.
    last_anim_tick
}

/// Render one frame to the terminal and update draw/tick bookkeeping.
fn draw_frame(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut AppState,
    size: TerminalSize,
    tick: u128,
    needs_draw: &mut bool,
    last_anim_tick: &mut u128,
) -> Result<()> {
    ensure_transcript_layout(app, size);
    let draw_started = Instant::now();
    terminal.draw(|frame| {
        render_main_view(frame, app);
    })?;
    if let Some(perf) = app.perf.as_mut() {
        perf.record_draw(draw_started.elapsed());
    }
    *needs_draw = false;
    *last_anim_tick = tick;
    Ok(())
}

/// Block (or non-block) for the next event, choosing the right wait
/// strategy based on whether we are working, have deferred lines, etc.
fn poll_next_event(
    ui_rx: &Receiver<UiEvent>,
    prefetched_events: &mut VecDeque<UiEvent>,
    deferred_server_lines: &VecDeque<String>,
    working: bool,
    app: &AppState,
    loop_now: Instant,
) -> ChannelOk<Option<UiEvent>> {
    if let Some(ev) = prefetched_events.pop_front() {
        return ChannelOk::Ok(Some(ev));
    }
    if !deferred_server_lines.is_empty() {
        return match ui_rx.try_recv() {
            Ok(ev) => ChannelOk::Ok(Some(ev)),
            Err(TryRecvError::Empty) => ChannelOk::Ok(None),
            Err(TryRecvError::Disconnected) => ChannelOk::Disconnected,
        };
    }
    if working {
        return match ui_rx.recv_timeout(animation_poll_timeout()) {
            Ok(ev) => ChannelOk::Ok(Some(ev)),
            Err(RecvTimeoutError::Timeout) => ChannelOk::Ok(None),
            Err(RecvTimeoutError::Disconnected) => ChannelOk::Disconnected,
        };
    }
    if let Some(wait) = app.pending_ralph_continuation_wait(loop_now) {
        return match ui_rx.recv_timeout(wait) {
            Ok(ev) => ChannelOk::Ok(Some(ev)),
            Err(RecvTimeoutError::Timeout) => ChannelOk::Ok(None),
            Err(RecvTimeoutError::Disconnected) => ChannelOk::Disconnected,
        };
    }
    match ui_rx.recv() {
        Ok(ev) => ChannelOk::Ok(Some(ev)),
        Err(_) => ChannelOk::Disconnected,
    }
}

/// Collect the first event plus any additional buffered events up to
/// the per-cycle drain limit.
fn gather_incoming_events(
    first: Option<UiEvent>,
    prefetched_events: &mut VecDeque<UiEvent>,
    ui_rx: &Receiver<UiEvent>,
) -> ChannelOk<Vec<UiEvent>> {
    let mut incoming = Vec::new();
    if let Some(ev) = first {
        incoming.push(ev);
    }
    while incoming.len() < MAX_UI_DRAIN_PER_CYCLE {
        if let Some(ev) = prefetched_events.pop_front() {
            incoming.push(ev);
            continue;
        }
        match ui_rx.try_recv() {
            Ok(ev) => incoming.push(ev),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => return ChannelOk::Disconnected,
        }
    }
    ChannelOk::Ok(incoming)
}

/// Compute the server budget and run event prioritization.
fn budgeted_events(
    incoming: Vec<UiEvent>,
    deferred_server_lines: &mut VecDeque<String>,
) -> Vec<UiEvent> {
    let has_terminal_input = incoming
        .iter()
        .any(|ev| matches!(ev, UiEvent::Terminal(_)));
    let server_budget = if has_terminal_input {
        SERVER_BUDGET_WITH_INPUT
    } else {
        SERVER_BUDGET_IDLE
    };
    prioritize_events(incoming, deferred_server_lines, server_budget)
}

/// Handle a single UI event. Returns `true` when the caller should quit.
fn process_event(
    event: &UiEvent,
    client: &dyn BackendClient,
    app: &mut AppState,
    size: TerminalSize,
    needs_draw: &mut bool,
) -> bool {
    let event_started = Instant::now();
    let quit = match event {
        UiEvent::ServerLine(line) => {
            handle_server_line(client, app, line);
            *needs_draw = true;
            false
        }
        UiEvent::Terminal(ev) => match handle_terminal_event(client, app, ev.clone(), size) {
            TerminalEventResult::Quit => true,
            TerminalEventResult::Continue {
                needs_draw: event_needs_draw,
            } => {
                if event_needs_draw {
                    *needs_draw = true;
                }
                false
            }
        },
    };
    if let Some(perf) = app.perf.as_mut() {
        perf.event_handle.push(event_started.elapsed());
    }
    quit
}

/// Process a single server JSON line, dispatching any resulting actions.
fn handle_server_line(client: &dyn BackendClient, app: &mut AppState, line: &str) {
    if let Some(perf) = app.perf.as_mut() {
        perf.notifications = perf.notifications.saturating_add(1);
    }
    if let Some(action) = handle_server_message_line(app, line) {
        match action {
            ServerRequestAction::ReplyError {
                request_id,
                code,
                message,
            } => {
                if let Err(err) = client.respond_error(&request_id, code, &message) {
                    app.set_status(format!("server request reply failed: {err}"));
                }
            }
        }
    }
}

pub(super) fn can_submit_queued_turn(
    working: bool,
    deferred_server_lines: &VecDeque<String>,
    prefetched_events: &VecDeque<UiEvent>,
) -> bool {
    !working
        && deferred_server_lines.is_empty()
        && !prefetched_events.iter().any(queued_turn_blocking_event)
}

fn queued_turn_blocking_event(event: &UiEvent) -> bool {
    match event {
        UiEvent::ServerLine(_) => true,
        UiEvent::Terminal(CrosstermEvent::Resize(_, _)) => false,
        UiEvent::Terminal(CrosstermEvent::Mouse(mouse)) => mouse.kind != MouseEventKind::Moved,
        UiEvent::Terminal(_) => true,
    }
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
