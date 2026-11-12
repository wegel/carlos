// Regression checks through the real receiver, queue rules, and app dispatch.

#[test]
fn repeated_reset_key_keeps_two_distinct_transcript_rows_through_dispatch() {
    use serde_json::json;

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
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let mut app = AppState::new(session.to_string());
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    let rows: Vec<_> = app.session.messages().iter().map(|m| m.text.as_str()).collect();
    assert_eq!(rows, ["first", "second"]);
    assert_eq!(
        app.item_message_index("claude-msg-0-1-0").expect("first row"), 0,
        "{:?}",
        app.session.messages().items()
    );
    assert_eq!(app.item_message_index("claude-msg-0-2-0").expect("second row"), 1);
    let stale = json!({"method":"claude/nativeResult","params":{"generation":1,"epoch":1,"subtype":"success"}});
    assert!(client
        .handle_event(&stale.to_string())
        .expect("handler")
        .is_empty());
}

#[test]
fn native_completions_follow_prompt_and_item_facts_in_real_dispatch() {
    use serde_json::json;

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
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"hello"}]}),
            crate::backend::PromptOrigin::Human,
            crate::backend::PromptPriority::Queue,
            Duration::from_secs(5),
        )
        .expect("prompt accepted");
    let response: Value = serde_json::from_str(&response).expect("reply");
    let id = response["result"]["turn"]["id"].as_str().expect("turn id");
    let mut app = AppState::new("session".into());
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert_eq!(
        app.session.messages()
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        ["hello", "answer"]
    );
    assert_eq!(
        app.session.turn_status_by_id()[id],
        crate::app::models::TurnStatus::Completed
    );
    assert!(app.session.active_turn_id().is_none());
    assert!(app.claude.prompts.entries.is_empty());
    assert_eq!(*app.session.status(), "turn completed");
}

fn native_sdk(message: Value) -> Value {
    serde_json::json!({"v":1,"type":"event","event":"sdk","message":message})
}

fn drain_probe_through_dispatch(
    client: &dyn BackendClient,
    receiver: &EventReceiver,
    app: &mut AppState,
) {
    client
        .request_native_control("probe", serde_json::json!({}))
        .expect("probe");
    client
        .call(
            "thread/name/set",
            serde_json::json!({"threadId":"session","name":"barrier"}),
            Duration::from_secs(5),
        )
        .expect("reader barrier");
    dispatch_received(client, app, receiver.try_iter().collect());
}

fn dispatch_received(client: &dyn BackendClient, app: &mut AppState, lines: Vec<String>) {
    use std::collections::VecDeque;

    use crate::app::input::prioritize_events;
    use crate::event::UiEvent;

    let mut deferred = VecDeque::new();
    let mut events = prioritize_events(
        lines.into_iter().map(UiEvent::ServerLine).collect(),
        &mut deferred,
        2,
    );
    loop {
        for event in events {
            let mut draw = false;
            assert!(!crate::app::input::input_dispatch::process_event(
                event,
                client,
                app,
                crate::app::TerminalSize {
                    width: 100,
                    height: 30
                },
                &mut draw
            ));
        }
        if deferred.is_empty() {
            break;
        }
        events = prioritize_events(Vec::new(), &mut deferred, 2);
    }
}

#[test]
fn eof_after_repeated_reset_keys_closes_the_app_at_the_current_epoch() {
    use serde_json::json;

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
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let mut app = AppState::new(session.into());
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    client
        .request_native_control("shutdown", json!({}))
        .expect("helper exit");
    dispatch_until_eof(&client, &receiver, &mut app);
    assert!(app.session.claude().phase() == crate::app::claude_state::ClaudePhase::Closed);
    assert_eq!(
        app.session.messages().last().expect("EOF error").text,
        "Claude controller stdout closed unexpectedly"
    );
}

#[test]
fn notices_keep_native_duplicate_keys_and_stop_before_translating_embedded_events() {
    use serde_json::json;

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
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let mut app = AppState::new(session.into());
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages()[0].text, "only once");
    assert_eq!(app.claude.activity.thinking, Some(42));
}

#[test]
fn non_system_init_and_browser_subtypes_still_translate_their_sdk_items() {
    use serde_json::json;

    let mut frames = Vec::new();
    for subtype in ["init", "elicitation_complete"] {
        frames.push(native_sdk(json!({"type":"assistant","subtype":subtype,"message":{"id":subtype,"role":"assistant","content":[{"type":"text","text":subtype}]}})));
    }
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let mut app = AppState::new("session".into());
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    assert_eq!(
        app.session.messages()
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        ["init", "elicitation_complete"]
    );
}

#[test]
fn malformed_history_keeps_earlier_rows_and_closes_only_the_app_query() {
    use serde_json::json;

    let session = "00000000-0000-4000-8000-000000000001";
    let history = json!({"messages":[
        {"source":"human","message":{"type":"user","session_id":session,"uuid":"human","message":{"content":"retained history"}}},
        {"message":{"type":"assistant","session_id":"wrong"}}
    ]});
    let (client, receiver) = crate::claude_backend::receiver_client(Vec::new(), history, 1024);
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
    let mut app = AppState::new(session.into());
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert_eq!(
        app.session.messages()
            .iter()
            .map(|m| m.text.as_str())
            .collect::<Vec<_>>(),
        [
            "retained history",
            "Claude history belongs to a different session"
        ]
    );
    assert!(app.session.claude().phase() == crate::app::claude_state::ClaudePhase::Closed);
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
    use serde_json::json;

    let frames = vec![
        json!({"v":1,"type":"event","event":"error","error":{"message":"native failure"}}),
        json!({"v":1,"type":"event","event":"stderr","text":"native stderr"}),
    ];
    let (client, receiver) = crate::claude_backend::receiver_client(frames, json!({}), 1024);
    let mut app = AppState::new("session".into());
    drain_probe_through_dispatch(&client, &receiver, &mut app);
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"accepted before UI closed"}]}),
            crate::backend::PromptOrigin::Human,
            crate::backend::PromptPriority::Queue,
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
    assert_eq!(
        app.session.turn_status_by_id()[id],
        crate::app::models::TurnStatus::Failed
    );
    assert!(app.session.messages().iter().any(|m| m.text == "native stderr"));
    assert!(app.claude.prompts.entries.is_empty());
}

fn dispatch_until_eof(client: &dyn BackendClient, receiver: &EventReceiver, app: &mut AppState) {
    loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("EOF fact");
        let value: Value = serde_json::from_str(&line).expect("JSON");
        let done = value["method"] == "claude/error";
        dispatch_received(client, app, vec![line]);
        if done {
            break;
        }
    }
}

#[test]
fn failed_history_reply_is_delivered_instead_of_discarded_by_reader_closure() {
    use serde_json::json;

    let (client, receiver) = crate::claude_backend::receiver_client(
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
    let mut app = AppState::new("session".into());
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    assert!(app.session.claude().phase() == crate::app::claude_state::ClaudePhase::Closed);
    assert_eq!(app.session.messages()[0].text, "failed interrupt");
}

#[test]
fn missing_session_metadata_closes_handler_before_later_control_replies() {
    use serde_json::json;

    let session = "00000000-0000-4000-8000-000000000001";
    let (client, receiver) = crate::claude_backend::receiver_client(
        Vec::new(),
        json!({"_failedMethods":["sessions.read"]}),
        1024,
    );
    client
        .request_native_control("sessions.info", json!({"sessionId":session}))
        .expect("metadata read");
    reader_barrier(&client);
    let mut app = AppState::new(session.into());
    dispatch_received(&client, &mut app, receiver.try_iter().collect());
    let response = client
        .submit_prompt(
            "turn/start",
            json!({"input":[{"text":"late accepted prompt"}]}),
            crate::backend::PromptOrigin::Human,
            crate::backend::PromptPriority::Queue,
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
        !app.session.turn_status_by_id().contains_key(id),
        "main ignores controller replies after closing the query"
    );
    assert_eq!(
        app.session.messages()[0].text,
        "The requested Claude session was not found"
    );
    client
        .request_native_control("shutdown", json!({}))
        .expect("helper exit");
    dispatch_until_eof(&client, &receiver, &mut app);
    assert_eq!(
        app.session.turn_status_by_id()[id],
        crate::app::models::TurnStatus::Failed
    );
}

fn reader_barrier(client: &dyn BackendClient) {
    client
        .call(
            "thread/name/set",
            serde_json::json!({"threadId":"session","name":"barrier"}),
            Duration::from_secs(5),
        )
        .expect("reader barrier");
}
