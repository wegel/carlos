//! Conversation event loop: poll terminal and server events, dispatch to handlers.

#[path = "input_queue_rules.rs"]
mod input_queue_rules;

#[path = "input_dispatch.rs"]
mod input_dispatch;

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError, TryRecvError};
use std::time::Instant;

use anyhow::Result;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use self::input_dispatch::process_event;
#[cfg(test)]
pub(super) use self::input_queue_rules::is_priority_server_line;
pub(super) use self::input_queue_rules::{can_submit_queued_turn, prioritize_events};
use super::input_events::ensure_transcript_layout;
#[cfg(test)]
pub(super) use super::input_events::is_mobile_mouse_key_candidate;
use super::notifications::{animation_poll_timeout, animation_tick};
use super::render::render_main_view;
use super::terminal_ui::with_terminal;
use super::turn_submit::submit_dequeued_turn_text;
use super::{AppState, TerminalSize};
use crate::backend::BackendClient;
#[cfg(not(feature = "dictation"))]
use crate::event::spawn_event_forwarders;
#[cfg(feature = "dictation")]
use crate::event::spawn_event_forwarders_with_dictation;
use crate::event::UiEvent;

const MAX_UI_DRAIN_PER_CYCLE: usize = 4096;
const SERVER_BUDGET_WITH_INPUT: usize = 8;
const SERVER_BUDGET_IDLE: usize = 256;

// --- Types ---

/// Mutable state carried across loop iterations.
struct LoopState {
    deferred: VecDeque<String>,
    prefetched: VecDeque<UiEvent>,
    needs_draw: bool,
    last_anim_tick: u128,
}

impl Default for LoopState {
    fn default() -> Self {
        Self {
            deferred: VecDeque::new(),
            prefetched: VecDeque::new(),
            needs_draw: true,
            last_anim_tick: 0,
        }
    }
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

/// Outcome of trying to collect a batch of events for this loop iteration.
enum EventBatchOutcome {
    Disconnected,
    Empty,
    Events(Vec<UiEvent>),
}

// --- Public API ---

pub(super) fn make_input_area() -> ratatui_textarea::TextArea<'static> {
    ratatui_textarea::TextArea::default()
}

pub(crate) fn run_conversation_tui(
    client: &dyn BackendClient,
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Result<()> {
    with_terminal(|terminal| {
        let ui_rx = wire_event_forwarders(app, server_events_rx);
        let mut state = LoopState::default();
        loop {
            if let Some(p) = app.perf.as_mut() {
                p.loop_count = p.loop_count.saturating_add(1);
            }
            let exit = run_one_loop_iteration(terminal, client, app, &ui_rx, &mut state)?;
            if exit {
                return Ok(());
            }
        }
    })
}

// --- Loop iteration ---

/// Run a single iteration of the event loop. Returns `true` to exit the loop.
fn run_one_loop_iteration(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    client: &dyn BackendClient,
    app: &mut AppState,
    ui_rx: &Receiver<UiEvent>,
    state: &mut LoopState,
) -> Result<bool> {
    let size = terminal_size(terminal)?;
    let loop_now = Instant::now();
    let working = app.active_turn_id.is_some();

    if drain_prefetched_idle(ui_rx, working, &state.deferred, &mut state.prefetched).disconnected()
    {
        return Ok(true);
    }
    if try_submit_queued_turn(
        client,
        app,
        working,
        &state.deferred,
        &state.prefetched,
        loop_now,
    ) {
        state.needs_draw = true;
        return Ok(false);
    }
    state.last_anim_tick =
        update_animation_state(working, state.last_anim_tick, &mut state.needs_draw);
    maybe_draw(terminal, app, size, working, state)?;

    let events = match collect_loop_events(ui_rx, app, working, loop_now, state) {
        EventBatchOutcome::Disconnected => return Ok(true),
        EventBatchOutcome::Empty => return Ok(false),
        EventBatchOutcome::Events(v) => v,
    };
    for ev in events {
        if process_event(ev, client, app, size, &mut state.needs_draw) {
            return Ok(true);
        }
    }
    // Working state may have changed inside process_event (turn started or completed),
    // so recompute it before deciding whether to animate the next frame.
    let working_after_events = app.active_turn_id.is_some();
    maybe_draw(terminal, app, size, working_after_events, state)?;
    Ok(false)
}

#[cfg(feature = "dictation")]
fn wire_event_forwarders(
    app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Receiver<UiEvent> {
    let (dictation_tx, dictation_rx) = std::sync::mpsc::channel();
    app.configure_dictation_event_sender(dictation_tx);
    spawn_event_forwarders_with_dictation(server_events_rx, dictation_rx)
}

#[cfg(not(feature = "dictation"))]
fn wire_event_forwarders(
    _app: &mut AppState,
    server_events_rx: std::sync::mpsc::Receiver<String>,
) -> Receiver<UiEvent> {
    spawn_event_forwarders(server_events_rx)
}

fn maybe_draw(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut AppState,
    size: TerminalSize,
    working: bool,
    state: &mut LoopState,
) -> Result<()> {
    if !state.needs_draw {
        return Ok(());
    }
    let tick = if working { animation_tick() } else { 0 };
    draw_frame(
        terminal,
        app,
        size,
        tick,
        &mut state.needs_draw,
        &mut state.last_anim_tick,
    )
}

fn collect_loop_events(
    ui_rx: &Receiver<UiEvent>,
    app: &mut AppState,
    working: bool,
    loop_now: Instant,
    state: &mut LoopState,
) -> EventBatchOutcome {
    let next_event = match poll_next_event(
        ui_rx,
        &mut state.prefetched,
        &state.deferred,
        working,
        app,
        loop_now,
    ) {
        ChannelOk::Disconnected => return EventBatchOutcome::Disconnected,
        ChannelOk::Ok(ev) => ev,
    };
    if let Some(p) = app.perf.as_mut() {
        p.poll_wait.push(loop_now.elapsed());
    }
    let incoming = match gather_incoming_events(next_event, &mut state.prefetched, ui_rx) {
        ChannelOk::Disconnected => return EventBatchOutcome::Disconnected,
        ChannelOk::Ok(v) => v,
    };
    let events = budgeted_events(incoming, &mut state.deferred);
    if events.is_empty() {
        EventBatchOutcome::Empty
    } else {
        EventBatchOutcome::Events(events)
    }
}

// --- Frame and turn helpers ---

fn terminal_size(terminal: &Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<TerminalSize> {
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
        submit_dequeued_turn_text(client, app, next_turn.text, next_turn.record_input_history);
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

// --- Channel polling ---

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
    let has_terminal_input = incoming.iter().any(|ev| matches!(ev, UiEvent::Terminal(_)));
    let server_budget = if has_terminal_input {
        SERVER_BUDGET_WITH_INPUT
    } else {
        SERVER_BUDGET_IDLE
    };
    prioritize_events(incoming, deferred_server_lines, server_budget)
}
