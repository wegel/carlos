//! Send through captured native Claude commands while Core owns echo, queue and history.

use std::time::Instant;

use serde_json::json;

use super::command_work::BackendCall;
use super::models::Role;
use super::prompt_commands::{PromptContext, PromptOptions};
use super::state::SessionState;
use crate::backend::{BackendClient, BackendKind, PromptOrigin};
use crate::claude_backend::receiver_client;

fn submit(state: &mut SessionState, text: &str, record_history: bool) -> Option<&'static str> {
    let (client, _receiver) = receiver_client(Vec::new(), json!({}), 1024);
    state.claude.generation = client
        .connection_generation()
        .expect("actual helper generation");
    let prepared = state
        .prepare_turn_text(
            BackendKind::Claude,
            text.into(),
            PromptOptions {
                record_history,
                force_start: false,
                origin: PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare original native send");
    let work = prepared.work?;
    let method = match &work.call {
        BackendCall::Prompt { method, params, .. } => {
            assert_eq!(params["input"][0]["text"], text);
            *method
        }
        _ => panic!("a prompt must prepare exactly one native send"),
    };
    let completion = work.execute(&client.capture_commands().expect("captured Claude writer"));
    let native = completion
        .result
        .as_ref()
        .expect("native helper accepted prompt");
    uuid::Uuid::parse_str(
        native["result"]["turn"]["id"]
            .as_str()
            .expect("native UUID"),
    )
    .expect("actual native prompt UUID");
    let finished = state
        .finish_backend_work(completion)
        .expect("settle originating send");
    assert!(finished.work.is_none(), "the acceptance cannot resend");
    Some(method)
}

#[test]
fn submit_turn_text_sends_claude_steer_when_turn_active() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.set_active_turn_id(Some("turn-1".to_string()));

    let method = submit(&mut app, "follow up", true);

    assert_eq!(method, Some("turn/steer"));
    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::User);
    assert_eq!(app.messages()[0].text, "follow up");
    assert_eq!(app.input_history().history_len(), 1);
    assert_eq!(app.status(), "sent steer");
}

#[test]
fn submit_turn_text_queues_behind_existing_claude_turns() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_turn_input("first");

    let method = submit(&mut app, "second", true);

    let first = app
        .dequeue_turn_input(Instant::now())
        .expect("first queued turn");
    let second = app
        .dequeue_turn_input(Instant::now())
        .expect("second queued turn");
    assert_eq!(first.text, "first");
    assert_eq!(second.text, "second");
    assert_eq!(app.status(), "queued behind pending Claude turn");
    assert_eq!(method, None);
}

#[test]
fn submit_turn_text_does_not_queue_behind_delayed_ralph_continuation() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.queue_ralph_continuation("continue");

    let method = submit(&mut app, "user prompt", true);

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].text, "user prompt");
    assert_eq!(app.status(), "sent turn");
    assert_eq!(method, Some("turn/start"));
}

#[test]
fn submit_turn_text_appends_user_message_for_claude_start() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);

    let method = submit(&mut app, "hello", true);

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].role, Role::User);
    assert_eq!(app.messages()[0].text, "hello");
    assert_eq!(method, Some("turn/start"));
}

#[test]
fn submit_turn_text_marks_claude_turn_active_immediately() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);

    let method = submit(&mut app, "hello", true);

    uuid::Uuid::parse_str(
        app.active_turn_id()
            .as_deref()
            .expect("accepted native turn"),
    )
    .expect("active native UUID immediately after acceptance");
    assert_eq!(app.turn_start(), Some(1));
    assert_eq!(method, Some("turn/start"));
}

#[test]
fn submit_turn_text_backfills_existing_history_for_claude_start() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);
    app.record_prompt_history("hello", None);

    let method = submit(&mut app, "hello", true);

    assert_eq!(app.input_history().history_len(), 1);
    assert_eq!(
        app.input_history().message_ids(),
        &[Some(app.messages()[0].id)]
    );
    assert_eq!(method, Some("turn/start"));
}

#[test]
fn submit_turn_text_skips_history_for_internal_claude_turns() {
    let mut app = SessionState::new("thread-1".to_string(), Default::default(), false);

    let method = submit(&mut app, "auto", false);

    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.input_history().history_len(), 0);
    assert_eq!(method, Some("turn/start"));
}
