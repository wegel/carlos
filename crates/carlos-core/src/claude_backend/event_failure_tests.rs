//! Checks failure after partial fan-out and native clear fences at the actual receiver.

use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::{assert_eof, client, expected, sdk, SESSION};
use crate::backend::BackendClient;

#[test]
fn partial_enqueue_failure_settles_an_unpublished_completion_and_charges_final_facts() {
    let (client, receiver) = client(
        vec![
            sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"})),
            sdk(
                json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":"owned"}),
            ),
        ],
        json!({}),
        1,
    );
    client
        .handler
        .borrow_mut()
        .lifecycle
        .submitted("owned")
        .expect("register");
    client
        .helper_call("probe", json!({}), Duration::from_secs(2))
        .expect_err("full queue stops peer");
    let mut actual = Vec::new();
    while actual.len() < 3 {
        let line = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("final fact");
        client.acknowledge_event(line.len());
        actual.extend(
            client
                .handle_event(&line)
                .expect("Claude handler")
                .iter()
                .map(|line| serde_json::from_str::<Value>(line).expect("JSON")),
        );
    }
    assert_eq!(
        actual,
        vec![
            expected(
                "claude/activity",
                json!({"type":"system","subtype":"session_state_changed","state":"idle"})
            ),
            expected(
                "claude/error",
                json!({"error":{"message":"Claude UI event backlog overflowed; session stopped"}})
            ),
            expected(
                "turn/completed",
                json!({"turn":{"id":"owned","status":"failed"}})
            ),
        ]
    );
    assert_eof(&client, &receiver);
}

#[test]
fn duplicate_clear_and_old_conversation_text_cannot_cross_the_accepted_init() {
    let new = "00000000-0000-4000-8000-000000000002";
    let reset = sdk(
        json!({"type":"conversation_reset","session_id":SESSION,"uuid":"clear-once","new_conversation_id":"opaque"}),
    );
    let (client, receiver) = client(
        vec![
            sdk(json!({"type":"system","subtype":"init","session_id":SESSION})),
            reset.clone(),
            reset,
            sdk(
                json!({"type":"assistant","session_id":SESSION,"message":{"id":"old","role":"assistant","content":[{"type":"text","text":"old text"}]}}),
            ),
            sdk(json!({"type":"system","subtype":"init","session_id":new})),
            sdk(
                json!({"type":"assistant","session_id":new,"message":{"id":"new","role":"assistant","content":[{"type":"text","text":"new text"}]}}),
            ),
        ],
        json!({}),
        1024,
    );
    client
        .helper_call("probe", json!({}), Duration::from_secs(2))
        .expect("native frames");
    let actual = super::event_test_support::records(&client, &receiver);
    assert_eq!(
        actual
            .iter()
            .filter(|line| line["method"] == "claude/conversationReset")
            .count(),
        1
    );
    let items: Vec<&Value> = actual
        .iter()
        .filter(|line| line["method"] == "item/completed")
        .collect();
    assert_eq!(
        items,
        [
            &json!({"method":"item/completed","params":{"epoch":1,"generation":1,"item":{"id":"claude-msg-0-1-0","type":"agentMessage","text":"new text"},"sourceKey":{"sessionId":new,"kind":"assistant","id":"new","block":0}}})
        ]
    );
    assert!(!actual
        .iter()
        .any(|line| line["method"] == "turn/started" || line["method"] == "turn/completed"));
}

#[test]
fn a_retired_helper_keeps_its_generation_on_late_common_receiver_events() {
    use std::process::Command;
    use std::sync::Arc;

    let (client, receiver) = client(
        vec![sdk(
            json!({"type":"assistant","session_id":SESSION,"message":{"id":"late","role":"assistant","content":[{"type":"text","text":"old helper"}]}}),
        )],
        json!({}),
        16,
    );
    let old = client.transport.lock().expect("transport").clone();
    let next_events = client.events_tx.for_generation(2);
    let mut command = Command::new("python3");
    command.args(["-c","import sys,json\nfor line in sys.stdin:\n r=json.loads(line); print(json.dumps({'v':1,'type':'reply','id':r['id'],'ok':True,'result':{}}),flush=True)\n if r['method']=='shutdown': break\n"]);
    let next = super::transport::ControllerTransport::start_command(command, next_events.clone())
        .expect("replacement helper");
    next_events
        .send(
            json!({"method":"claude/sessionChanged","params":{"sessionId":"replacement"}})
                .to_string(),
        )
        .expect("announce new generation");
    *client.transport.lock().expect("replace transport") = Arc::new(next);
    old.call("probe", json!({}), Duration::from_secs(2))
        .expect("late old frames");
    let actual = super::event_test_support::records(&client, &receiver);
    assert_eq!(
        actual,
        vec![
            json!({"method":"claude/sessionChanged","params":{"generation":2,"sessionId":"replacement"}}),
            expected(
                "item/completed",
                json!({"item":{"id":"claude-msg-17-1-0","type":"agentMessage","text":"old helper"},"sourceKey":{"sessionId":SESSION,"kind":"assistant","id":"late","block":0}})
            ),
        ]
    );
}
