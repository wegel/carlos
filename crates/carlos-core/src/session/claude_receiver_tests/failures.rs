//! Keep real native receiver facts in FIFO order through the Core reducer.

use std::time::Duration;

use serde_json::{json, Value};

use super::{dispatch_received, dispatch_until_eof, drain_probe_through_dispatch, reader_barrier};
use crate::backend::{BackendClient, PromptOrigin, PromptPriority};
use crate::claude_backend::receiver_client;
use crate::session::claude::ClaudePhase;
use crate::session::models::TurnStatus;
use crate::session::state::SessionState;

#[test]
fn malformed_history_keeps_earlier_rows_and_closes_only_the_app_query() {
    let session = "00000000-0000-4000-8000-000000000001";
    let history = json!({"messages":[
        {"source":"human","message":{"type":"user","session_id":session,"uuid":"human","message":{"content":"retained history"}}},
        {"message":{"type":"assistant","session_id":"wrong"}}
    ]});
    let (client, receiver) = receiver_client(Vec::new(), history, 1024);
    client
        .request_native_control("sessions.read", json!({"sessionId":session}))
        .expect("history read");
    client
        .call(
            "thread/name/set",
            json!({"threadId":session,"name":"barrier"}),
            Duration::from_secs(5),
        )
        .expect("reader keeps serving calls");
    let mut app = SessionState::new(session.into(), Default::default(), false);
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert_eq!(
        app.messages()
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        [
            "retained history",
            "Claude history belongs to a different session"
        ]
    );
    assert!(app.claude().phase() == ClaudePhase::Closed);
    client
        .call(
            "thread/name/set",
            json!({"threadId":session,"name":"after error"}),
            Duration::from_secs(5),
        )
        .expect("history errors do not kill the helper");
}

#[test]
fn accepted_input_after_a_fatal_fact_is_settled_by_eof_and_stderr_stays_visible() {
    let frames = vec![
        json!({"v":1,"type":"event","event":"error","error":{"message":"native failure"}}),
        json!({"v":1,"type":"event","event":"stderr","text":"native stderr"}),
    ];
    let (client, receiver) = receiver_client(frames, json!({}), 1024);
    let mut app = SessionState::new("session".into(), Default::default(), false);
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"accepted before UI closed"}]}),
            PromptOrigin::Human,
            PromptPriority::Queue,
            Duration::from_secs(5),
        )
        .expect("helper accepted prompt");
    let response: Value = serde_json::from_str(&response).expect("reply");
    let id = response["result"]["turn"]["id"]
        .as_str()
        .expect("prompt id");
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    client
        .request_native_control("shutdown", json!({}))
        .expect("helper exit");
    dispatch_until_eof(&client, &receiver, &mut app);
    assert_eq!(app.turn_status_by_id()[id], TurnStatus::Failed);
    assert!(app.messages().iter().any(|m| m.text == "native stderr"));
    assert!(app.claude_prompts().next().is_none());
}

#[test]
fn failed_history_reply_is_delivered_instead_of_discarded_by_reader_closure() {
    let (client, receiver) = receiver_client(
        Vec::new(),
        json!({"_failedMethods":["sessions.read"]}),
        1024,
    );
    client
        .request_native_control("sessions.read", json!({"sessionId":"session"}))
        .expect("request");
    client
        .call(
            "thread/name/set",
            json!({"threadId":"session","name":"barrier"}),
            Duration::from_secs(5),
        )
        .expect("reader barrier");
    let mut app = SessionState::new("session".into(), Default::default(), false);
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert!(app.claude().phase() == ClaudePhase::Closed);
    assert_eq!(app.messages()[0].text, "failed interrupt");
}

#[test]
fn missing_session_metadata_closes_handler_before_later_control_replies() {
    let session = "00000000-0000-4000-8000-000000000001";
    let (client, receiver) = receiver_client(
        Vec::new(),
        json!({"_failedMethods":["sessions.read"]}),
        1024,
    );
    client
        .request_native_control("sessions.info", json!({"sessionId":session}))
        .expect("metadata read");
    reader_barrier(&client);
    let mut app = SessionState::new(session.into(), Default::default(), false);
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"late accepted prompt"}]}),
            PromptOrigin::Human,
            PromptPriority::Queue,
            Duration::from_secs(5),
        )
        .expect("accepted prompt");
    let response: Value = serde_json::from_str(&response).expect("response");
    let id = response["result"]["turn"]["id"]
        .as_str()
        .expect("prompt id");
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    client
        .request_native_control("sessions.read", json!({"sessionId":session}))
        .expect("late failed reply");
    reader_barrier(&client);
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert!(
        !app.turn_status_by_id().contains_key(id),
        "main ignores controller replies after closing the query"
    );
    assert_eq!(
        app.messages()[0].text,
        "The requested Claude session was not found"
    );
    client
        .request_native_control("shutdown", json!({}))
        .expect("helper exit");
    dispatch_until_eof(&client, &receiver, &mut app);
    assert_eq!(app.turn_status_by_id()[id], TurnStatus::Failed);
}
