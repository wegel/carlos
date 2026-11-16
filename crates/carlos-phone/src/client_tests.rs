//! Prove the typed boundary uses real client greeting rights and rejects stale socket callbacks.

use serde_json::Value;

use crate::test_support::{hello, token};
use crate::{PhoneClient, PhoneError};

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
        Err(PhoneError::InvalidMessage { .. })
    ));
    assert_eq!(client.maximum_message_bytes(), 8 * 1024 * 1024);
}
