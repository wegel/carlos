//! Read reconnect task facts without constructing an app or any terminal widget.

use serde_json::json;

use super::panel_snapshot::reconnect_panel;
use crate::session::test_backend::TestBackend;

#[test]
fn reconnect_panel_reads_only_native_facts_without_a_view() {
    let peer = TestBackend::default();
    peer.reads.lock().expect("reads").extend([
        (
            "thread/list".into(),
            json!({"result":{"data":[],"nextCursor":null}}),
        ),
        (
            "thread/backgroundTerminals/list".into(),
            json!({"result":{"data":[],"nextCursor":null}}),
        ),
    ]);
    let panel = reconnect_panel(&peer, "root").expect("complete panel");
    assert_eq!(
        panel,
        json!({"agents":[],"processes":[],"older":null,"error":null,
        "notice":"All verified descendants in this snapshot","verifiedOwners":[],"processScopes":{"root":null}})
    );
    let calls = peer.calls.lock().expect("calls");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].0, "thread/list");
    assert_eq!(calls[0].1["ancestorThreadId"], "root");
    assert_eq!(calls[0].1["limit"], 1);
    assert_eq!(calls[1].0, "thread/backgroundTerminals/list");
    assert_eq!(calls[1].1, json!({"threadId":"root","limit":1}));
    assert!(peer.reads.lock().expect("all replies consumed").is_empty());
}

#[test]
fn reconnect_verifies_off_window_parents_and_keeps_unavailable_scopes_distinct() {
    let peer = TestBackend::default();
    peer.reads.lock().expect("reads").extend([
        ("thread/list".into(), json!({"result":{"data":[
            {"id":"leaf","source":{"subAgent":{}},"parentThreadId":"parent","status":{"type":"active"}},
            {"id":"unrelated","source":{"subAgent":{}},"parentThreadId":"unrelated","status":{"type":"active"}}
        ]}})),
        ("thread/read".into(), json!({"result":{"thread":{
            "id":"parent","source":{"subAgent":{}},"parentThreadId":"root"}}})),
        ("thread/backgroundTerminals/list".into(), json!({"error":{"message":"root unavailable"}})),
        ("thread/backgroundTerminals/list".into(), json!({"result":{
            "data":[{"processId":"p1"}],"nextCursor":"next"}})),
        ("thread/backgroundTerminals/list".into(), json!({"result":{
            "data":[{"processId":"p2"}],"nextCursor":null}})),
    ]);
    let panel = reconnect_panel(&peer, "root").expect("complete verified window");
    assert_eq!(panel["agents"].as_array().expect("agents").len(), 1);
    assert_eq!(panel["agents"][0]["id"], "leaf");
    assert_eq!(
        panel["processes"],
        json!([
            {"thread":"leaf","data":{"processId":"p1"}},
            {"thread":"leaf","data":{"processId":"p2"}}
        ])
    );
    assert_eq!(
        panel["processScopes"],
        json!({"root":"root unavailable","leaf":null})
    );
    assert!(panel["error"]
        .as_str()
        .expect("unavailable notice")
        .contains("root unavailable"));
    let mut owners = panel["verifiedOwners"].as_array().expect("owners").clone();
    owners.sort_by_key(|owner| owner.as_str().expect("owner").to_string());
    assert_eq!(owners, vec![json!("leaf"), json!("parent")]);
    let calls = peer.calls.lock().expect("calls");
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls[1].1,
        json!({"threadId":"parent","includeTurns":false})
    );
    assert_eq!(calls[2].1["threadId"], "root");
    assert_eq!(calls[3].1["threadId"], "leaf");
    assert_eq!(
        calls[4].1,
        json!({"threadId":"leaf","limit":1,"cursor":"next"})
    );
    assert!(peer.reads.lock().expect("consumed replies").is_empty());
}
