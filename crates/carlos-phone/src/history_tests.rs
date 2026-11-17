//! Keep history rows and failed-read identities faithful across the native boundary.

use serde_json::{json, Value};

use crate::test_support::{connected, info, reply, request, SESSION};

fn row(index: u64) -> Value {
    json!({"id":format!("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb-{index:016x}"),
        "kind":"user","format":"plain","text":format!("native {index} 🌍"),"done":true})
}

#[test]
fn older_reads_keep_the_original_anchor_after_failure_and_return_checked_rows() {
    let client = connected();
    let subscription = client.subscribe(SESSION.into()).expect("subscribe");
    reply(
        &client,
        &subscription,
        json!({"kind":"snapshot","snapshot":{
            "info":info("ready"),"entries":[row(2)],"approvals":[],"seq":"1","has_older":true
        }}),
    );
    let failed = client.older_history(SESSION.into()).expect("older read");
    let first = request(&failed);
    assert_eq!(first["method"], "session/history");
    assert_eq!(first["params"]["before"], row(2)["id"]);
    let failure = client
        .receive(
            failed.generation,
            serde_json::to_vec(&json!({
                "jsonrpc":"2.0","id":failed.request_id,
                "error":{"code":-32000,"message":"native read failed"}
            }))
            .expect("error bytes"),
        )
        .expect("read failure");
    assert_eq!(failure.failures[0].request_id, Some(failed.request_id));
    assert_eq!(
        client.entry_ids(SESSION.into(), 0, 256).expect("unchanged"),
        [row(2)["id"].as_str().expect("row id")]
    );
    let retry = client
        .older_history(SESSION.into())
        .expect("explicit retry");
    assert_eq!(request(&retry)["params"], first["params"]);
    reply(
        &client,
        &retry,
        json!({"entries":[row(1)],"seq":"1","has_older":false}),
    );
    assert_eq!(
        client
            .entry_ids(SESSION.into(), 0, 256)
            .expect("ordered rows"),
        [
            row(1)["id"].as_str().expect("older id"),
            row(2)["id"].as_str().expect("original id")
        ]
    );
    assert!(
        !client
            .transcript(SESSION.into())
            .expect("metadata")
            .expect("subscribed")
            .has_older
    );
    assert!(client.operations().expect("no mutations").is_empty());
}
