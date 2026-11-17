//! Prove the typed boundary uses real client greeting rights and rejects stale socket callbacks.

use serde_json::Value;

use crate::test_support::{hello, reply, request, subscribed, token, SESSION};
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

#[test]
fn an_empty_reconnect_replay_notifies_the_phone_when_cached_controls_become_current() {
    let client = subscribed("ready");
    client.disconnect(1).expect("background socket closes");
    let greeting = client
        .begin("iPhone".into(), token())
        .expect("foreground reconnect");
    let welcome = client
        .receive(greeting.generation, hello(greeting.request_id, true))
        .expect("same private rights");
    let subscription = welcome
        .outgoing
        .into_iter()
        .find(|frame| request(frame)["method"] == "session/subscribe")
        .expect("resume the selected session");
    assert!(
        !client
            .transcript(SESSION.into())
            .expect("cached")
            .expect("metadata")
            .current_snapshot
    );
    let batch = reply(
        &client,
        &subscription,
        serde_json::json!({
            "kind":"replay", "events":[], "seq":"1"
        }),
    );
    assert!(
        client
            .transcript(SESSION.into())
            .expect("current")
            .expect("metadata")
            .ready
    );
    assert!(
        batch
            .sessions
            .iter()
            .any(|change| change.session == SESSION),
        "A native renderer must learn that an unchanged cached session is current again"
    );
    assert!(batch.outgoing.is_empty());
    assert!(client.operations().expect("no mutation replay").is_empty());
}
