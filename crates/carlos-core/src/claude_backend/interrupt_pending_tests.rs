//! Keep pending controls local to their captured prompts and retain one bounded terminal cause.

use serde_json::{json, Value};

use super::event_handler::EventHandler;
use super::event_normalizer::fact;

#[test]
fn an_unrelated_prompt_does_not_erase_an_earlier_result_idle_pair() {
    let mut handler = EventHandler::default();
    submit(&mut handler, "first");
    bind(&mut handler, "first");
    let intent = handler.lifecycle.begin_interrupt();
    result(&mut handler, "first");
    idle(&mut handler);
    submit(&mut handler, "other");
    bind(&mut handler, "other");
    handler.lifecycle.finish_interrupt(&intent, false);
    let completed = values(handler.take_control_events());
    assert_eq!(statuses(&completed), [("first".into(), "completed".into())]);
    result(&mut handler, "other");
    let completed = idle(&mut handler);
    assert_eq!(statuses(&completed), [("other".into(), "completed".into())]);
}

#[test]
fn unrelated_results_complete_while_the_captured_success_waits() {
    let mut handler = EventHandler::default();
    submit(&mut handler, "first");
    bind(&mut handler, "first");
    let intent = handler.lifecycle.begin_interrupt();
    result(&mut handler, "first");
    idle(&mut handler);
    submit(&mut handler, "other");
    bind(&mut handler, "other");
    result(&mut handler, "other");
    assert_eq!(
        statuses(&idle(&mut handler)),
        [("other".into(), "completed".into())]
    );
    handler.lifecycle.finish_interrupt(&intent, true);
    assert_eq!(
        statuses(&values(handler.take_control_events())),
        [("first".into(), "interrupted".into())]
    );
}

#[test]
fn overlapping_controls_preserve_acceptance_but_a_later_sequential_attempt_keeps_legacy_rules() {
    for overlapping in [true, false] {
        let mut handler = EventHandler::default();
        submit(&mut handler, "first");
        bind(&mut handler, "first");
        let first = handler.lifecycle.begin_interrupt();
        let second = overlapping.then(|| handler.lifecycle.begin_interrupt());
        handler.lifecycle.finish_interrupt(&first, true);
        let second = second.unwrap_or_else(|| handler.lifecycle.begin_interrupt());
        handler.lifecycle.finish_interrupt(&second, false);
        result(&mut handler, "first");
        let expected = if overlapping {
            "interrupted"
        } else {
            "completed"
        };
        assert_eq!(
            statuses(&idle(&mut handler)),
            [("first".into(), expected.into())]
        );
    }
}

#[test]
fn a_terminal_retains_only_its_first_bounded_cause_and_fails_unrelated_unfinished_work() {
    let mut handler = EventHandler::default();
    submit(&mut handler, "first");
    bind(&mut handler, "first");
    let intent = handler.lifecycle.begin_interrupt();
    result(&mut handler, "first");
    idle(&mut handler);
    submit(&mut handler, "other");
    let cause = format!("first cause {}", "x".repeat(100_000));
    let failed = handle(
        &mut handler,
        "claude/error",
        json!({"error":{"message":cause},"unused":"y".repeat(100_000)}),
    );
    assert_eq!(statuses(&failed), [("other".into(), "failed".into())]);
    assert!(handler.defers_terminal());
    assert!(handle(
        &mut handler,
        "claude/error",
        json!({"error":{"message":"later EOF"}})
    )
    .is_empty());
    assert!(handle(
        &mut handler,
        "claude/callback",
        json!({"callbackId":"late"})
    )
    .is_empty());
    assert!(handler.take_control_events().is_empty());
    handler.lifecycle.finish_interrupt(&intent, false);
    let finished = values(handler.take_control_events());
    assert_eq!(statuses(&finished), [("first".into(), "completed".into())]);
    let terminal = finished.last().expect("first closure after completion");
    let message = terminal["params"]["error"]["message"]
        .as_str()
        .expect("bounded diagnostic");
    assert!(message.starts_with("first cause"));
    assert!(message.len() <= 512 + " [details truncated]".len());
    assert!(
        terminal.to_string().len() < 1024,
        "unused native payload was not retained"
    );
    assert_eq!(terminal["params"]["generation"], 1);
    assert_eq!(terminal["params"]["epoch"], 0);
    assert!(!handler.defers_terminal());
    assert!(handler.take_control_events().is_empty());
}

fn submit(handler: &mut EventHandler, id: &str) {
    handle(handler, "claude/promptSubmitted", json!({"uuid":id}));
}

fn bind(handler: &mut EventHandler, id: &str) {
    handle(
        handler,
        "claude/nativeBind",
        json!({"user_message_uuid":id}),
    );
}

fn result(handler: &mut EventHandler, id: &str) {
    handle(
        handler,
        "claude/nativeResult",
        json!({"user_message_uuid":id,"subtype":"success","is_error":false,"error":null}),
    );
}

fn idle(handler: &mut EventHandler) -> Vec<Value> {
    handle(handler, "claude/activity", json!({"state":"idle"}))
}

fn handle(handler: &mut EventHandler, method: &str, params: Value) -> Vec<Value> {
    values(handler.handle(&fact(method, params)))
}

fn values(lines: Vec<String>) -> Vec<Value> {
    lines
        .iter()
        .map(|line| serde_json::from_str(line).expect("normalized fact"))
        .collect()
}

fn statuses(values: &[Value]) -> Vec<(String, String)> {
    values
        .iter()
        .filter(|value| value["method"] == "turn/completed")
        .map(|value| {
            (
                value["params"]["turn"]["id"]
                    .as_str()
                    .expect("prompt")
                    .into(),
                value["params"]["turn"]["status"]
                    .as_str()
                    .expect("status")
                    .into(),
            )
        })
        .collect()
}
