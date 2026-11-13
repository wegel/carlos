//! Checks the old split between ignored live SDK errors and fatal controller errors.

use serde_json::{json, Value};

use super::event_race_support::DEADLINE;
use super::event_test_support::{client, records, sdk, stream, SESSION};

#[test]
fn malformed_sdk_delta_is_ignored_and_pending_call_receives_success() {
    for index in [None, Some(json!("wrong")), Some(Value::Null)] {
        let mut delta =
            json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"bad"}});
        if let Some(index) = index {
            delta["index"] = index;
        }
        let (client, receiver) = client(vec![stream(delta)], json!({"answer":"success"}), 1024);
        let result = client
            .helper_call("probe", json!({}), DEADLINE)
            .expect("SDK error must not reject call");
        assert_eq!(result["answer"], "success");
        assert!(records(&client, &receiver).is_empty());
    }
}

#[test]
fn invalid_controller_frames_reject_pending_calls_and_report_fatal_error() {
    for frame in [
        json!({"v":2,"type":"event","event":"sdk","message":{}}),
        sdk(json!({"type":"system","subtype":"init","session_id":"invalid"})),
        sdk(json!({"type":"conversation_reset","session_id":SESSION})),
    ] {
        let (client, receiver) = client(vec![frame], json!({"answer":"success"}), 1024);
        assert!(client.helper_call("probe", json!({}), DEADLINE).is_err());
        let line = receiver.recv_timeout(DEADLINE).expect("fatal error");
        let value: Value = serde_json::from_str(&line).expect("JSON");
        assert_eq!(value["method"], "claude/error");
    }
}

#[test]
fn eof_is_suppressed_by_a_closed_flag_even_when_the_frame_type_is_unknown() {
    use super::event_race_support::{notify, transport};
    use crate::backend::BackendClient;

    let (client, receiver) = client(vec![json!({"v":1,"event":"closed"})], json!({}), 1024);
    client
        .helper_call("probe", json!({}), DEADLINE)
        .expect("ignored frame");
    let transport = transport(&client);
    let done = notify(&transport.events.hooks, "reader_done");
    client
        .request_native_control("shutdown", json!({}))
        .expect("helper exit");
    done.recv_timeout(DEADLINE).expect("EOF was handled");
    let actual = records(&client, &receiver);
    assert!(!actual
        .iter()
        .any(|record| record["method"] == "claude/error"));
}

#[test]
fn controller_failure_and_subsequent_eof_both_reach_the_receiver() {
    use super::event_race_support::{notify, transport};

    let (client, receiver) = client(vec![json!({"v":2})], json!({}), 1024);
    let transport = transport(&client);
    let done = notify(&transport.events.hooks, "reader_done");
    client
        .helper_call("probe", json!({}), DEADLINE)
        .expect_err("bad controller envelope");
    done.recv_timeout(DEADLINE).expect("both errors ordered");
    let actual = records(&client, &receiver);
    let errors: Vec<_> = actual
        .iter()
        .map(|v| v["params"]["error"]["message"].as_str().expect("error"))
        .collect();
    assert_eq!(
        errors,
        [
            "Claude controller sent an invalid protocol frame",
            "Claude controller stdout closed unexpectedly"
        ]
    );
}
