//! Event queue coalescing rules: drop redundant mouse motion and resize events.

use std::collections::VecDeque;

use crossterm::event::{Event as CrosstermEvent, MouseEventKind};
use serde_json::Value;

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
