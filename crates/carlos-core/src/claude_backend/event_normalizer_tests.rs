//! Proves the complete ordered receiver records without constructing an app or terminal.

use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::{client, expected, records, submit, SESSION};
use crate::backend::BackendClient;

#[test]
fn receiver_emits_reasoning_text_tools_sources_and_one_owned_completion() {
    let (client, receiver) = client(super::event_fixture::live_frames(), json!({}), 1024);
    let id = submit(&client);
    let mut actual = records(&client, &receiver);
    assert_eq!(
        actual.remove(0),
        json!({"method":"claude/promptSubmitted","params":{"generation":1,"uuid":id,"text":"hello","inputKind":"prompt","origin":"human","priority":"later"}})
    );
    assert_eq!(actual, super::event_fixture::live_records(&id));
}

#[test]
fn history_has_native_keys_authors_and_an_independent_cursor_before_start() {
    let message = json!({"type":"assistant","session_id":SESSION,"message":{"id":"answer","role":"assistant","content":[{"type":"text","text":"past"}]}});
    let user =
        json!({"type":"user","session_id":SESSION,"uuid":"human","message":{"content":"hello"}});
    let (client, receiver) = client(
        Vec::new(),
        json!({"messages":[{"source":"human","message":user},{"source":"system","message":message}],"nextOffset":null}),
        1024,
    );
    client
        .request_native_control("sessions.read", json!({"sessionId":SESSION}))
        .expect("read history");
    let mut actual = Vec::new();
    loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("history event");
        client.acknowledge_event(line.len());
        let value: Value = serde_json::from_str(&line).expect("JSON");
        let done = value["method"] == "claude/controlReply";
        actual.push(value);
        if done {
            break;
        }
    }
    assert_eq!(
        actual,
        vec![
            expected("claude/historyRecord", json!({"record":user})),
            expected(
                "claude/historyPrompt",
                json!({"uuid":"human","text":"hello","source":"human","inputKind":null,"sourceKey":{"sessionId":SESSION,"kind":"user","id":"human"}})
            ),
            expected("claude/historyRecord", json!({"record":message})),
            expected(
                "item/completed",
                json!({"item":{"id":"claude-msg-0-2-0","type":"agentMessage","text":"past"},"history":true,"sourceKey":{"sessionId":SESSION,"kind":"assistant","id":"answer","block":0}})
            ),
            expected(
                "claude/controlReply",
                json!({"ok":true,"result":{"nextOffset":null},"error":{"message":"failed interrupt"},"requestMethod":"sessions.read","requestContext":{"revision":null,"contextToken":null,"taskId":null,"runId":null}})
            ),
        ]
    );
    let transport = client.transport.lock().expect("transport").clone();
    assert_eq!(
        transport
            .normalizer
            .lock()
            .expect("normalizer")
            .live
            .current_message_seq,
        0
    );
}
