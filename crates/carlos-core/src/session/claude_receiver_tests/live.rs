//! Keep real native receiver facts in FIFO order through the Core reducer.

use std::time::Duration;

use serde_json::{json, Value};

use super::{dispatch_received, dispatch_until_eof, drain_probe_through_dispatch, native_sdk};
use crate::backend::{BackendClient, PromptOrigin, PromptPriority};
use crate::claude_backend::receiver_client;
use crate::session::claude::ClaudePhase;
use crate::session::models::TurnStatus;
use crate::session::state::SessionState;

#[test]
fn repeated_reset_key_keeps_two_distinct_transcript_rows_through_dispatch() {
    let session = "00000000-0000-4000-8000-000000000001";
    let mut frames = vec![native_sdk(
        json!({"type":"system","subtype":"init","session_id":session}),
    )];
    for (uuid, text) in [("reset-1", "first"), ("reset-2", "second")] {
        frames.extend([
            native_sdk(json!({"type":"conversation_reset","session_id":session,"uuid":uuid,"new_conversation_id":"same"})),
            native_sdk(json!({"type":"system","subtype":"init","session_id":session})),
            native_sdk(json!({"type":"assistant","message":{"id":text,"role":"assistant","content":[{"type":"text","text":text}]}})),
        ]);
    }
    let (client, receiver) = receiver_client(frames, json!({}), 1024);
    let mut app = SessionState::new(session.to_string(), Default::default(), false);
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    let rows: Vec<_> = app.messages().iter().map(|m| m.text.as_str()).collect();
    assert_eq!(rows, ["first", "second"]);
    assert_eq!(
        app.item_message_index(&app.backend_entry_key("claude-msg-0-1-0"))
            .expect("first row"),
        0,
        "{:?}",
        app.messages().items()
    );
    assert_eq!(
        app.item_message_index(&app.backend_entry_key("claude-msg-0-2-0"))
            .expect("second row"),
        1
    );
    let stale = json!({"method":"claude/nativeResult","params":{"generation":1,"epoch":1,"subtype":"success"}});
    assert!(client
        .handle_event(&stale.to_string())
        .expect("handler")
        .is_empty());
}

#[test]
fn native_completions_follow_prompt_and_item_facts_in_real_dispatch() {
    let frames = vec![
        native_sdk(
            json!({"type":"system","subtype":"init","session_id":"00000000-0000-4000-8000-000000000001","user_message_uuid":"$prompt"}),
        ),
        native_sdk(
            json!({"type":"assistant","user_message_uuid":"$prompt","message":{"role":"assistant","content":[{"type":"text","text":"answer"}]}}),
        ),
        native_sdk(
            json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":"$prompt"}),
        ),
        native_sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"})),
    ];
    let (client, receiver) = receiver_client(frames, json!({}), 1024);
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"hello"}]}),
            PromptOrigin::Human,
            PromptPriority::Queue,
            Duration::from_secs(5),
        )
        .expect("prompt accepted");
    let response: Value = serde_json::from_str(&response).expect("reply");
    let id = response["result"]["turn"]["id"].as_str().expect("turn id");
    let mut app = SessionState::new("session".into(), Default::default(), false);
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert_eq!(
        app.messages()
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        ["hello", "answer"]
    );
    assert_eq!(app.turn_status_by_id()[id], TurnStatus::Completed);
    assert!(app.active_turn_id().is_none());
    assert!(app.claude_prompts().next().is_none());
    assert_eq!(app.status(), "turn completed");
}

#[test]
fn eof_after_repeated_reset_keys_closes_the_app_at_the_current_epoch() {
    let session = "00000000-0000-4000-8000-000000000001";
    let frames = vec![
        native_sdk(json!({"type":"system","subtype":"init","session_id":session})),
        native_sdk(
            json!({"type":"conversation_reset","session_id":session,"uuid":"one","new_conversation_id":"same"}),
        ),
        native_sdk(json!({"type":"system","subtype":"init","session_id":session})),
        native_sdk(
            json!({"type":"conversation_reset","session_id":session,"uuid":"two","new_conversation_id":"same"}),
        ),
        native_sdk(json!({"type":"system","subtype":"init","session_id":session})),
    ];
    let (client, receiver) = receiver_client(frames, json!({}), 1024);
    let mut app = SessionState::new(session.into(), Default::default(), false);
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    client
        .request_native_control("shutdown", json!({}))
        .expect("helper exit");
    dispatch_until_eof(&client, &receiver, &mut app);
    assert!(app.claude().phase() == ClaudePhase::Closed);
    assert_eq!(
        app.messages().last().expect("EOF error").text,
        "Claude controller stdout closed unexpectedly"
    );
}

#[test]
fn notices_keep_native_duplicate_keys_and_stop_before_translating_embedded_events() {
    let session = "00000000-0000-4000-8000-000000000001";
    let notice = native_sdk(
        json!({"type":"system","subtype":"informational","content":"only once","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":"not an SDK item"}}}),
    );
    let mut frames = vec![native_sdk(
        json!({"type":"system","subtype":"init","session_id":session}),
    )];
    for uuid in ["one", "two"] {
        frames.extend([
            native_sdk(json!({"type":"conversation_reset","session_id":session,"uuid":uuid,"new_conversation_id":"same"})),
            native_sdk(json!({"type":"system","subtype":"init","session_id":session})),
            native_sdk(json!({"type":"system","subtype":"thinking_tokens","estimated_tokens":42})),
            notice.clone(),
        ]);
    }
    let (client, receiver) = receiver_client(frames, json!({}), 1024);
    let mut app = SessionState::new(session.into(), Default::default(), false);
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    assert_eq!(app.messages().len(), 1);
    assert_eq!(app.messages()[0].text, "only once");
    assert_eq!(app.claude().activity().thinking(), Some(42));
}

#[test]
fn non_system_init_and_browser_subtypes_still_translate_their_sdk_items() {
    let mut frames = Vec::new();
    for subtype in ["init", "elicitation_complete"] {
        frames.push(native_sdk(json!({"type":"assistant","subtype":subtype,"message":{"id":subtype,"role":"assistant","content":[{"type":"text","text":subtype}]}})));
    }
    let (client, receiver) = receiver_client(frames, json!({}), 1024);
    let mut app = SessionState::new("session".into(), Default::default(), false);
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    assert_eq!(
        app.messages()
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        ["init", "elicitation_complete"]
    );
}
