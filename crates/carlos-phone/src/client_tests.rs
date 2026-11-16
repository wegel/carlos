//! Prove the typed boundary uses real client greeting rights and rejects stale socket callbacks.

use serde_json::{json, Value};

use crate::{PhoneClient, PhoneError};

fn token() -> String {
    format!("{}.{}", "1".repeat(32), "2".repeat(64))
}

fn hello(id: i64, resumed: bool) -> Vec<u8> {
    serde_json::to_vec(&json!({"jsonrpc":"2.0","id":id,"result":{
        "protocol":1,"client":"3".repeat(32),"instance":"4".repeat(32),
        "resume_token":"5".repeat(64),"next_operation":"1","resumed":resumed,
        "host":"fixture","daemon_version":"0.1.0"
    }}))
    .expect("greeting reply")
}

#[test]
fn greeting_and_reconnect_preserve_private_rights_without_replaying_work() {
    let client = PhoneClient::new();
    let first = client.begin("iPhone".into(), token()).expect("begin");
    let frame: Value = serde_json::from_str(&first.payload).expect("request");
    assert_eq!(frame["method"], "hello");
    assert_eq!(frame["params"]["token"], token());
    let batch = client
        .receive(first.generation, hello(first.request_id, false))
        .expect("welcome");
    assert!(batch.ready);
    assert!(batch.outgoing.is_empty());
    client.disconnect(first.generation).expect("closed socket");
    let next = client.begin("iPhone".into(), token()).expect("reconnect");
    let frame: Value = serde_json::from_str(&next.payload).expect("request");
    assert_eq!(frame["params"]["resume"], "3".repeat(32));
    assert_eq!(frame["params"]["resume_token"], "5".repeat(64));
    assert!(client
        .receive(first.generation, b"invalid obsolete frame".to_vec())
        .expect("old callback ignored")
        .outgoing
        .is_empty());
    client
        .disconnect(first.generation)
        .expect("late old close ignored");
    let batch = client
        .receive(next.generation, hello(next.request_id, true))
        .expect("resumed");
    assert!(batch.ready);
    assert!(
        batch.outgoing.is_empty(),
        "reconnect does not invent a mutation"
    );
}

#[test]
fn invalid_credentials_and_frames_have_typed_errors_without_exposing_a_secret() {
    let client = PhoneClient::new();
    let secret = "PRIVATE invalid token";
    let error = client
        .begin("iPhone".into(), secret.into())
        .err()
        .expect("invalid token");
    assert!(matches!(error, PhoneError::InvalidInput { .. }));
    assert!(!error.to_string().contains(secret));
    let first = client.begin("iPhone".into(), token()).expect("valid token");
    assert!(matches!(
        client.receive(first.generation, b"invalid frame".to_vec()),
        Err(PhoneError::Protocol { .. })
    ));
    assert_eq!(client.maximum_message_bytes(), 8 * 1024 * 1024);
}
