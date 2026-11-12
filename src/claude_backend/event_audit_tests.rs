//! Covers reset races and process facts whose old owner ran on the app thread.

use std::thread;

use serde_json::{json, Value};

use super::event_race_support::{notify, pause_once, peer, receive, request, transport, DEADLINE};
use super::event_test_support::{client, records, sdk, SESSION};
use crate::backend::BackendClient;

#[test]
fn history_continuation_after_a_paused_reset_cannot_release_old_pieces() {
    let (client, receiver) = peer();
    client
        .helper_call("init", json!({}), DEADLINE)
        .expect("native init");
    records(&client, &receiver);
    client
        .request_native_control("sessions.read", json!({"sessionId":SESSION}))
        .expect("history");
    assert_eq!(
        receive(&client.events_tx, &receiver)["method"],
        "claude/historyFactPart"
    );
    client
        .helper_call("barrier", json!({}), DEADLINE)
        .expect("history batch has released its permit");
    let transport = transport(&client);
    let reset = pause_once(&transport.events.hooks, "batch_line");
    let history_ready = notify(&transport.events.hooks, "history_ready");
    request(&client, "reset");
    reset.wait();
    thread::scope(|scope| {
        let history = scope.spawn(|| transport.emitter.continue_history(Some(0)));
        history_ready
            .recv_timeout(DEADLINE)
            .expect("continuation reached gate");
        reset.resume();
        history
            .join()
            .expect("history caller")
            .expect("stale continuation ignored");
    });
    client
        .helper_call("barrier", json!({}), DEADLINE)
        .expect("reader barrier");
    let actual = records(&client, &receiver);
    assert!(actual
        .iter()
        .any(|line| line["method"] == "claude/conversationReset" && line["params"]["epoch"] == 1));
    assert!(actual
        .iter()
        .all(|line| line["method"] != "claude/historyFactPart"));
    assert_eq!(client.events_tx.charged_bytes(), 0);
}

#[test]
fn process_facts_keep_native_order_on_either_side_of_reset_and_child_text_stays_hidden() {
    for reset_first in [false, true] {
        let reset = sdk(
            json!({"type":"conversation_reset","session_id":SESSION,"uuid":"reset","new_conversation_id":"next"}),
        );
        let mut frames = vec![sdk(
            json!({"type":"system","subtype":"init","session_id":SESSION}),
        )];
        let process = vec![
            sdk(
                json!({"type":"assistant","agent_id":"child","message":{"role":"assistant","content":[{"type":"text","text":"hidden"}]}}),
            ),
            sdk(
                json!({"type":"system","subtype":"task_progress","session_id":SESSION,"task_id":"child","run_id":"run"}),
            ),
            sdk(
                json!({"type":"system","subtype":"elicitation_complete","elicitation_id":"browser"}),
            ),
            json!({"v":1,"type":"callback","callbackId":"child-permission","kind":"permission","payload":{}}),
            json!({"v":1,"type":"event","event":"callback_cancelled","callbackId":"child-permission"}),
        ];
        if reset_first {
            frames.push(reset.clone());
        }
        frames.extend(process);
        if !reset_first {
            frames.push(reset);
        }
        let (client, receiver) = client(frames, json!({}), 1024);
        client
            .helper_call("probe", json!({}), DEADLINE)
            .expect("all native frames");
        let actual = records(&client, &receiver);
        assert_process_order(&actual, reset_first);
        assert!(actual
            .iter()
            .all(|line| !line.to_string().contains("hidden")));
        assert_eq!(client.events_tx.charged_bytes(), 0);
    }
}

#[test]
fn context_reply_retains_its_request_tokens_on_both_sides_of_compaction() {
    for context_first in [false, true] {
        let mut owner = super::event_normalizer::EventNormalizer::default();
        let context = json!({"type":"reply","ok":true,"result":{"totalTokens":33,"maxTokens":100}});
        let compact = sdk(json!({"type":"system","subtype":"compact_boundary","uuid":"compact"}));
        let tokens = json!({"revision":7,"contextToken":9,"taskId":null,"runId":null});
        let events = if context_first {
            [&context, &compact]
        } else {
            [&compact, &context]
        };
        let mut actual = Vec::new();
        for frame in events {
            let request = (frame["type"] == "reply").then_some("context");
            actual.extend(
                owner
                    .route(frame, request, &tokens, 0)
                    .expect("native frame"),
            );
        }
        let actual: Vec<Value> = actual
            .iter()
            .map(|line| serde_json::from_str(line).expect("JSON"))
            .collect();
        let usage = actual
            .iter()
            .find(|line| line["method"] == "thread/tokenUsage/updated")
            .expect("usage");
        assert_eq!(usage["params"]["requestContext"], tokens);
        assert_eq!(usage["params"]["tokenUsage"]["total"]["totalTokens"], 33);
        assert_eq!(
            actual[usize::from(context_first)]["method"],
            "claude/status"
        );
    }
}

fn assert_process_order(actual: &[Value], reset_first: bool) {
    let methods: Vec<&str> = actual
        .iter()
        .map(|line| line["method"].as_str().expect("method"))
        .collect();
    let mut expected = vec!["claude/notice", "claude/initialized", "thread/initialized"];
    if reset_first {
        expected.push("claude/conversationReset");
    }
    expected.extend([
        "claude/task",
        "claude/browserComplete",
        "claude/callback",
        "claude/callbackCancelled",
    ]);
    if !reset_first {
        expected.push("claude/conversationReset");
    }
    assert_eq!(methods, expected);
    for line in actual.iter().filter(|line| {
        [
            "claude/task",
            "claude/browserComplete",
            "claude/callback",
            "claude/callbackCancelled",
        ]
        .contains(&line["method"].as_str().expect("method"))
    }) {
        assert_eq!(line["params"]["epoch"], u64::from(reset_first));
    }
}
