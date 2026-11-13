//! Checks native identity changes and task/callback lifetimes without a CLI process.

use serde_json::{json, Value};

use super::conversation::ConversationGuard;

fn event(message: Value) -> Value {
    json!({"v":1,"type":"event","event":"sdk","message":message})
}
fn init(id: &str) -> Value {
    event(json!({"type":"system","subtype":"init","session_id":id}))
}
const OLD: &str = "11111111-1111-4111-8111-111111111111";
const NEW: &str = "22222222-2222-4222-8222-222222222222";

#[test]
fn observed_reset_opens_one_transition_while_callbacks_and_old_session_tasks_remain_live() {
    let mut guard = ConversationGuard::default();
    guard.route(init(OLD)).expect("first init");
    let reset = event(
        json!({"type":"conversation_reset","trigger":"clear","uuid":"reset","session_id":OLD,"new_conversation_id":"opaque-ui-key"}),
    );
    assert_eq!(guard.route(reset.clone()).expect("reset")[0].1, 1);
    assert!(guard.route(reset).expect("duplicate reset").is_empty());
    assert_eq!(
        guard
            .route(json!({"type":"callback","callbackId":"real-id"}))
            .expect("live callback")
            .len(),
        1
    );
    assert_eq!(guard.route(event(json!({"type":"system","subtype":"background_tasks_changed","session_id":OLD,"tasks":[]}))).expect("old task level").len(),1);
    assert!(guard
        .route(event(json!({"type":"result","session_id":OLD})))
        .expect("hold old result")
        .is_empty());
    assert_eq!(
        guard
            .route(init(NEW))
            .expect("authoritative native init")
            .len(),
        1
    );
    assert_eq!(guard.expected.as_deref(), Some(NEW));
    assert!(guard
        .route(init(OLD))
        .expect_err("no second permit")
        .to_string()
        .contains("session stopped"));
}

#[test]
fn foreign_reset_cannot_relax_resume_identity_and_transition_buffer_is_bounded() {
    let mut guard = ConversationGuard::default();
    guard.expected = Some(OLD.to_string());
    assert!(guard.route(event(json!({"type":"conversation_reset","trigger":"clear","uuid":"foreign","session_id":NEW,"new_conversation_id":"opaque"}))).expect("ignore foreign reset").is_empty());
    assert!(guard.route(init(NEW)).is_err());
    guard.route(event(json!({"type":"conversation_reset","trigger":"clear","uuid":"reset","session_id":OLD,"new_conversation_id":"opaque"}))).expect("reset");
    let oversized =
        event(json!({"type":"assistant","session_id":NEW,"text":"x".repeat(8*1024*1024)}));
    assert!(guard
        .route(oversized)
        .expect_err("bounded transition")
        .to_string()
        .contains("bounded"));
}

#[test]
fn reset_trigger_is_informational_and_missing_or_future_values_still_open_the_guard() {
    for trigger in [Value::Null, json!("future_native_reset")] {
        let mut guard = ConversationGuard::default();
        guard.route(init(OLD)).expect("initial identity");
        let mut reset = json!({"type":"conversation_reset","uuid":"reset","session_id":OLD,"new_conversation_id":"opaque"});
        if !trigger.is_null() {
            reset["trigger"] = trigger;
        }
        assert_eq!(
            guard
                .route(event(reset))
                .expect("reset without known trigger")
                .len(),
            1
        );
        guard.route(init(NEW)).expect("actual next identity");
        assert_eq!(guard.expected.as_deref(), Some(NEW));
    }
}

#[test]
fn child_init_and_reset_frames_never_change_main_identity_or_open_reset_permits() {
    let mut guard = ConversationGuard::default();
    guard.route(init(OLD)).expect("main init");
    for tags in [
        json!({"agent_id":"child"}),
        json!({"parent_agent_id":"parent"}),
        json!({"isSidechain":true}),
    ] {
        for mut message in [
            json!({"type":"system","subtype":"init","session_id":NEW}),
            json!({"type":"conversation_reset","session_id":OLD,"uuid":"child-reset","new_conversation_id":"child-key"}),
        ] {
            message
                .as_object_mut()
                .expect("message")
                .extend(tags.as_object().expect("tags").clone());
            assert_eq!(
                guard
                    .route(event(message))
                    .expect("preserve native child frame")
                    .len(),
                1
            );
            assert_eq!(guard.expected.as_deref(), Some(OLD));
            assert_eq!(guard.epoch, 0);
        }
    }
    assert!(guard.route(init(NEW)).is_err());
}
