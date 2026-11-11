//! Keep native Codex facts in causal order while terminal input stays responsive.

use std::collections::VecDeque;

use serde_json::Value;

use crate::event::UiEvent;

/// Budget native FIFO frames; only the terminal failure notice may bypass earlier work.
pub(in crate::app) fn order(
    incoming: Vec<UiEvent>,
    deferred: &mut VecDeque<String>,
    budget: usize,
) -> Vec<UiEvent> {
    let mut ready = Vec::new();
    for event in incoming {
        match event {
            UiEvent::ServerLine(line) if fatal(&line) => ready.push(UiEvent::ServerLine(line)),
            UiEvent::ServerLine(line) => deferred.push_back(line),
            input => ready.push(input),
        }
    }
    for _ in 0..budget {
        let Some(line) = deferred.pop_front() else {
            break;
        };
        ready.push(UiEvent::ServerLine(line));
    }
    ready
}

fn fatal(line: &str) -> bool {
    serde_json::from_str::<Value>(line).is_ok_and(|frame| {
        frame["method"] == "carlos/codex/closed"
            || frame["method"] == "carlos/codex/frame"
                && frame["params"]["event"]["method"] == "carlos/codex/closed"
    })
}
