//! Event queue coalescing rules: drop redundant mouse motion and resize events.

use std::collections::VecDeque;
use std::time::Instant;

use crossterm::event::{Event as CrosstermEvent, MouseEventKind};
use serde_json::Value;

use crate::app::turn_submit::submit_dequeued_turn_text;
use crate::app::AppState;
use crate::backend::BackendClient;
use crate::event::UiEvent;

pub(in crate::app) fn can_submit_queued_turn(
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
        UiEvent::Summary(_) => false,
        #[cfg(feature = "dictation")]
        UiEvent::Dictation(_) => true,
        UiEvent::Terminal(CrosstermEvent::Resize(_, _)) => false,
        UiEvent::Terminal(CrosstermEvent::Mouse(mouse)) => mouse.kind != MouseEventKind::Moved,
        UiEvent::Terminal(_) => true,
    }
}

pub(in crate::app) fn prioritize_events(
    incoming_events: Vec<UiEvent>,
    deferred_server_lines: &mut VecDeque<String>,
    server_budget: usize,
) -> Vec<UiEvent> {
    let mut prioritized = Vec::with_capacity(incoming_events.len());
    let mut priority_server_lines: Vec<String> = Vec::new();
    for event in incoming_events {
        match event {
            UiEvent::Summary(event) => prioritized.push(UiEvent::Summary(event)),
            UiEvent::Terminal(ev) => prioritized.push(UiEvent::Terminal(ev)),
            #[cfg(feature = "dictation")]
            UiEvent::Dictation(event) => prioritized.push(UiEvent::Dictation(event)),
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

pub(in crate::app) fn is_priority_server_line(line: &str) -> bool {
    let Ok(parsed) = serde_json::from_str::<Value>(line) else {
        return false;
    };
    let Some(method) = parsed.get("method").and_then(Value::as_str) else {
        return false;
    };
    matches!(method, "turn/completed" | "turn/started" | "error")
}

/// Attempt to submit a queued turn if conditions allow. Returns true
/// if a turn was submitted (caller should `continue` the loop).
pub(in crate::app) fn try_submit_queued_turn(
    client: &dyn BackendClient,
    app: &mut AppState,
    working: bool,
    deferred_server_lines: &VecDeque<String>,
    prefetched_events: &VecDeque<UiEvent>,
    now: Instant,
) -> bool {
    if client.kind() == crate::backend::BackendKind::Claude
        && app.claude.phase != crate::app::claude_state::ClaudePhase::Ready
    {
        return false;
    }
    if !can_submit_queued_turn(working, deferred_server_lines, prefetched_events) {
        return false;
    }
    if !crate::app::codex_queue::allows_automatic(app) {
        return false;
    }
    if app.ralph_runtime.next_input_origin(now) == Some(crate::backend::PromptOrigin::Automatic)
        && crate::app::codex_goals::ralph_error(app).is_some()
    {
        return false;
    }
    if let Some(next_turn) = app.dequeue_turn_input(now) {
        submit_dequeued_turn_text(
            client,
            app,
            next_turn.text,
            next_turn.record_input_history,
            next_turn.origin,
        );
        return true;
    }
    false
}
