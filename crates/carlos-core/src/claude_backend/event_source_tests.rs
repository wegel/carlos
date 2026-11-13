//! Matches native evidence across live and history cursors without trusting generated item IDs.

use std::time::Duration;

use serde_json::{json, Value};

use super::event_test_support::{client, records, sdk, SESSION};
use crate::backend::BackendClient;

#[test]
fn per_block_snapshots_keep_the_same_source_keys_in_live_and_history() {
    let snapshots = vec![
        json!({"type":"assistant","session_id":SESSION,"uuid":"thought","message":{"id":"native","role":"assistant","content":[{"type":"thinking","thinking":"why"}]}}),
        json!({"type":"assistant","session_id":SESSION,"uuid":"answer","message":{"id":"native","role":"assistant","content":[{"type":"text","text":"answer"}]}}),
    ];
    let (live, history) = live_and_history(snapshots);
    assert_eq!(live, history);
    assert_eq!(
        live,
        vec![
            json!({"sessionId":SESSION,"kind":"assistant","id":"native","block":0}),
            json!({"sessionId":SESSION,"kind":"assistant","id":"native","block":1})
        ]
    );
}

#[test]
fn consecutive_uuid_only_snapshots_keep_source_keys_in_live_and_history() {
    let snapshots = vec![
        json!({"type":"assistant","session_id":SESSION,"uuid":"first","message":{"role":"assistant","content":[{"type":"text","text":"one"}]}}),
        json!({"type":"assistant","session_id":SESSION,"uuid":"second","message":{"role":"assistant","content":[{"type":"thinking","thinking":"two"}]}}),
        json!({"type":"assistant","session_id":SESSION,"uuid":"third","message":{"role":"assistant","content":[{"type":"text","text":"three"},{"type":"thinking","thinking":"four"}]}}),
        json!({"type":"assistant","session_id":SESSION,"uuid":"fourth","message":{"role":"assistant","content":[{"type":"text","text":"five"}]}}),
    ];
    let (live, history) = live_and_history(snapshots);
    let expected = vec![
        json!({"sessionId":SESSION,"kind":"assistant","id":"first","block":0}),
        json!({"sessionId":SESSION,"kind":"assistant","id":"second","block":0}),
        json!({"sessionId":SESSION,"kind":"assistant","id":"third","block":0}),
        json!({"sessionId":SESSION,"kind":"assistant","id":"third","block":1}),
        json!({"sessionId":SESSION,"kind":"assistant","id":"fourth","block":0}),
    ];
    assert_eq!(live, expected);
    assert_eq!(history, expected);
}

#[test]
fn a_tool_id_that_looks_generated_does_not_steal_an_assistant_source_key() {
    let (client, receiver) = client(
        vec![sdk(
            json!({"type":"assistant","session_id":SESSION,"message":{"id":"native","role":"assistant","content":[
        {"type":"thinking","thinking":"why"},{"type":"tool_use","id":"claude-msg-17-1-0","name":"Bash","input":{"command":"pwd"}}]}}),
        )],
        json!({}),
        16,
    );
    client
        .helper_call("probe", json!({}), Duration::from_secs(2))
        .expect("native snapshot");
    assert_eq!(
        source_keys(records(&client, &receiver)),
        vec![
            json!({"sessionId":SESSION,"kind":"assistant","id":"native","block":0}),
            json!({"sessionId":SESSION,"kind":"tool","id":"claude-msg-17-1-0","part":"call"}),
        ]
    );
}

#[test]
fn records_without_native_message_evidence_cannot_promise_identity_on_reload() {
    let (client, receiver) = client(
        vec![sdk(
            json!({"type":"assistant","session_id":SESSION,"message":{"role":"assistant","content":[{"type":"text","text":"unkeyed"}]}}),
        )],
        json!({}),
        16,
    );
    client
        .helper_call("probe", json!({}), Duration::from_secs(2))
        .expect("unkeyed snapshot");
    let items: Vec<Value> = records(&client, &receiver)
        .into_iter()
        .filter(|record| record["method"] == "item/completed")
        .collect();
    assert_eq!(items.len(), 1);
    assert!(
        items[0]["params"].get("sourceKey").is_none(),
        "Milestone 2 must give this record a fresh id on replacement"
    );
}

fn live_and_history(snapshots: Vec<Value>) -> (Vec<Value>, Vec<Value>) {
    let (client, receiver) = client(
        snapshots.iter().cloned().map(sdk).collect(),
        json!({"messages":snapshots.iter().map(|message| json!({"source":"system","message":message})).collect::<Vec<_>>(),"nextOffset":null}),
        1024,
    );
    client
        .helper_call("probe", json!({}), Duration::from_secs(2))
        .expect("live snapshots");
    let live = source_keys(records(&client, &receiver));
    client
        .request_native_control("sessions.read", json!({"sessionId":SESSION}))
        .expect("history");
    let mut history = Vec::new();
    loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("history fact");
        client.acknowledge_event(line.len());
        let record: Value = serde_json::from_str(&line).expect("JSON");
        let done = record["method"] == "claude/controlReply";
        history.push(record);
        if done {
            break;
        }
    }
    (live, source_keys(history))
}

fn source_keys(records: Vec<Value>) -> Vec<Value> {
    records
        .into_iter()
        .filter(|record| record["method"] == "item/completed")
        .map(|record| record["params"]["sourceKey"].clone())
        .collect()
}
