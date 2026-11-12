//! Checks native result/idle permutations and real client interrupt controls at the receiver.

use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::{client, expected, records, scripted, sdk, submit};
use crate::backend::BackendClient;

#[test]
fn result_and_idle_permutations_complete_once_with_native_error_precedence() {
    for idle_first in [true, false] {
        for (is_error, reason, status) in [
            (false, "completed", "completed"),
            (true, "failed", "failed"),
            (true, "aborted_tools", "interrupted"),
        ] {
            let result = sdk(
                json!({"type":"result","subtype":"success","is_error":is_error,"terminal_reason":reason,"user_message_uuid":"$prompt"}),
            );
            let idle =
                sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"}));
            let frames = if idle_first {
                vec![idle.clone(), result.clone(), result, idle]
            } else {
                vec![result.clone(), idle.clone(), result, idle]
            };
            let (client, receiver) = client(frames, json!({}), 1024);
            let id = submit(&client);
            let completions: Vec<Value> = records(&client, &receiver)
                .into_iter()
                .filter(|record| record["method"] == "turn/completed")
                .collect();
            assert_eq!(
                completions,
                [expected(
                    "turn/completed",
                    json!({"turn":{"id":id,"status":status}})
                )]
            );
        }
    }
}

#[test]
fn a_real_failed_interrupt_restores_the_captured_intent_before_later_results() {
    let (client, receiver) = scripted(
        json!({
            "prompt":[sdk(json!({"type":"assistant","user_message_uuid":"$prompt","message":{"role":"assistant","content":[]}}))],
            "probe":[sdk(json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":"$prompt"})), sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"}))]
        }),
        json!({}),
        1024,
    );
    let id = submit(&client);
    records(&client, &receiver);
    client
        .call("turn/interrupt", json!({}), Duration::from_secs(2))
        .expect_err("failed native interrupt");
    client
        .helper_call("probe", json!({}), Duration::from_secs(2))
        .expect("later native idle");
    let completions: Vec<Value> = records(&client, &receiver)
        .into_iter()
        .filter(|record| record["method"] == "turn/completed")
        .collect();
    assert_eq!(
        completions,
        [expected(
            "turn/completed",
            json!({"turn":{"id":id,"status":"completed"}})
        )]
    );
}

#[test]
fn context_answers_emit_common_usage_with_the_exact_freshness_tokens() {
    let (client, receiver) = client(
        Vec::new(),
        json!({"totalTokens":123,"rawMaxTokens":456,"maxTokens":400}),
        16,
    );
    client
        .request_native_control("context", json!({"revision":7,"contextToken":9}))
        .expect("context read");
    let line = receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("usage");
    client.acknowledge_event(line.len());
    assert_eq!(
        serde_json::from_str::<Value>(&line).expect("JSON"),
        expected(
            "thread/tokenUsage/updated",
            json!({
                "tokenUsage":{"total":{"totalTokens":123},"modelContextWindow":456},
                "claudeContext":{"totalTokens":123,"rawMaxTokens":456,"maxTokens":400},
                "requestContext":{"revision":7,"contextToken":9,"taskId":null,"runId":null}
            })
        )
    );
}

#[test]
fn an_acknowledged_interrupt_preserves_native_failure_precedence() {
    for (is_error, status) in [(false, "interrupted"), (true, "failed")] {
        let (client, receiver) = scripted(
            json!({
                "prompt":[sdk(json!({"type":"assistant","user_message_uuid":"$prompt","message":{"role":"assistant","content":[]}}))],
                "probe":[sdk(json!({"type":"result","subtype":"success","is_error":is_error,"user_message_uuid":"$prompt"})),sdk(json!({"type":"system","subtype":"session_state_changed","state":"idle"}))]
            }),
            json!({"_interruptOk":true}),
            16,
        );
        let id = submit(&client);
        records(&client, &receiver);
        client
            .call("turn/interrupt", json!({}), Duration::from_secs(2))
            .expect("native interrupt accepted");
        client
            .helper_call("probe", json!({}), Duration::from_secs(2))
            .expect("native result");
        let completions: Vec<Value> = records(&client, &receiver)
            .into_iter()
            .filter(|record| record["method"] == "turn/completed")
            .collect();
        assert_eq!(
            completions,
            [expected(
                "turn/completed",
                json!({"turn":{"id":id,"status":status}})
            )]
        );
    }
}
