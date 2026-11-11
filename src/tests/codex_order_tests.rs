//! Keep Codex item, completion, settings and queue facts ordered across UI frame budgets.

use std::collections::VecDeque;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use crate::app::input::input_codex_order::order;
use crate::event::UiEvent;

fn event(method: &str) -> UiEvent {
    UiEvent::ServerLine(json!({"method":method,"params":{}}).to_string())
}

#[test]
fn keyboard_stays_first_but_server_facts_keep_fifo_across_small_budgets() {
    let mut deferred = VecDeque::new();
    let mut incoming = vec![
        event("item/completed"),
        event("turn/completed"),
        event("thread/queue/changed"),
        event("thread/settings/updated"),
        event("carlos/codex/controlReply"),
    ];
    incoming.push(UiEvent::Terminal(Event::Key(KeyEvent::new(
        KeyCode::Char('x'),
        KeyModifiers::NONE,
    ))));
    let mut observed = Vec::new();
    let first = order(incoming, &mut deferred, 1);
    assert!(matches!(first.first(), Some(UiEvent::Terminal(_))));
    observed.extend(first.into_iter().filter_map(|event| match event {
        UiEvent::ServerLine(line) => Some(line),
        _ => None,
    }));
    while !deferred.is_empty() {
        observed.extend(
            order(Vec::new(), &mut deferred, 1)
                .into_iter()
                .filter_map(|event| match event {
                    UiEvent::ServerLine(line) => Some(line),
                    _ => None,
                }),
        );
    }
    let methods = observed
        .iter()
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("frame")["method"]
                .as_str()
                .expect("method")
                .to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        methods,
        [
            "item/completed",
            "turn/completed",
            "thread/queue/changed",
            "thread/settings/updated",
            "carlos/codex/controlReply"
        ]
    );
}

#[test]
fn only_the_terminal_closed_diagnostic_bypasses_codex_backlog() {
    let mut deferred = VecDeque::from([json!({"method":"item/completed","params":{}}).to_string()]);
    let ready = order(
        vec![event("turn/completed"), event("carlos/codex/closed")],
        &mut deferred,
        1,
    );
    let UiEvent::ServerLine(first) = &ready[0] else {
        panic!("closed diagnostic");
    };
    assert!(first.contains("carlos/codex/closed"));
    assert_eq!(deferred.len(), 1);
    assert!(deferred[0].contains("turn/completed"));
}
