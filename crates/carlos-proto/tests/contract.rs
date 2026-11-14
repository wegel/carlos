//! Client envelope and identifier compatibility checks.

use carlos_proto::commands::{check_protocol, Command, Request};
use carlos_proto::events::NotificationMessage;
use carlos_proto::ids::{EntryId, Sequence, SessionId};
use serde_json::{from_value, json, to_value};

#[test]
fn rejects_malformed_or_noncanonical_addresses() {
    for value in ["", "a", "é", "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"] {
        assert!(from_value::<SessionId>(json!(value)).is_err());
    }
    let id: EntryId = format!("{}-0000000000000001", "a".repeat(32))
        .parse()
        .expect("entry");
    assert_eq!(to_value(&id).expect("encode"), json!(id.as_str()));
}

#[test]
fn cursors_preserve_all_bits_and_never_wrap() {
    let cursor = Sequence::new(u64::MAX);
    assert_eq!(
        from_value::<Sequence>(to_value(cursor).expect("encode")).expect("decode"),
        cursor
    );
    assert!(cursor.checked_next().is_none());
    for value in [
        json!(1),
        json!("01"),
        json!("+1"),
        json!("-1"),
        json!("18446744073709551616"),
    ] {
        assert!(from_value::<Sequence>(value).is_err());
    }
}

#[test]
fn source_byte_positions_and_lengths_preserve_all_bits_as_decimal_strings() {
    use carlos_proto::bytes::{ByteLength, ByteOffset};
    assert_eq!(
        to_value(ByteOffset::new(u64::MAX)).expect("offset"),
        json!("18446744073709551615")
    );
    assert_eq!(
        from_value::<ByteLength>(json!("18446744073709551615"))
            .expect("length")
            .get(),
        u64::MAX
    );
    for invalid in [
        json!(1),
        json!("01"),
        json!("-1"),
        json!("18446744073709551616"),
    ] {
        assert!(from_value::<ByteOffset>(invalid.clone()).is_err());
        assert!(from_value::<ByteLength>(invalid).is_err());
    }
}

#[test]
fn requests_roundtrip_and_enforce_rpc_version() {
    let value = json!({"jsonrpc":"2.0","id":"hello-1","method":"hello",
        "params":{"protocol":1,"client":"test","token":"device-token"}});
    let request: Request = from_value(value.clone()).expect("request");
    assert!(matches!(
        request.command,
        Command::Hello { protocol: 1, .. }
    ));
    assert_eq!(to_value(request).expect("encode"), value);
    let mut bad = value;
    bad["jsonrpc"] = json!("1.0");
    assert!(from_value::<Request>(bad).is_err());
    assert!(check_protocol(1).is_ok());
    assert_eq!(check_protocol(2).expect_err("mismatch").code, -32001);
}

#[test]
fn unknown_commands_and_invalid_request_ids_fail() {
    for id in [json!(null), json!(1.5), json!(true)] {
        assert!(from_value::<Request>(
            json!({"jsonrpc":"2.0","id":id,"method":"sessions/watch","params":{}})
        )
        .is_err());
    }
    assert!(
        from_value::<Request>(json!({"jsonrpc":"2.0","id":1,"method":"unknown","params":{}}))
            .is_err()
    );
}

#[test]
fn every_provisional_command_and_event_roundtrips_the_v1_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/v1.json")).expect("fixture");
    for request in fixture["requests"].as_array().expect("requests") {
        let parsed: Request = from_value(request.clone()).expect("v1 request");
        assert_eq!(to_value(parsed).expect("encode request"), *request);
    }
    for notification in fixture["notifications"].as_array().expect("notifications") {
        let parsed: NotificationMessage =
            from_value(notification.clone()).expect("v1 notification");
        assert_eq!(
            to_value(parsed).expect("encode notification"),
            *notification
        );
    }
    let mut unknown = fixture["notifications"][0].clone();
    unknown["params"]["event"]["kind"] = json!("future_event");
    assert!(from_value::<NotificationMessage>(unknown).is_err());
}

#[test]
fn output_tokens_are_bounded_and_revisions_preserve_all_bits() {
    use carlos_proto::output::{OutputCursor, OutputRevision};
    for invalid in [
        "".to_owned(),
        "x".repeat(257),
        "has space".into(),
        "é".into(),
    ] {
        assert!(from_value::<OutputCursor>(json!(invalid)).is_err());
    }
    let revision = OutputRevision::new(u64::MAX);
    assert_eq!(
        to_value(revision).expect("encode"),
        json!("18446744073709551615")
    );
    assert_eq!(
        from_value::<OutputRevision>(json!("18446744073709551615")).expect("decode"),
        revision
    );
}

#[test]
fn operation_counters_are_lossless_nonzero_and_canonical() {
    use carlos_proto::ids::OperationId;
    let last = OperationId::new(u64::MAX).expect("nonzero");
    assert_eq!(
        to_value(last).expect("encode"),
        json!("18446744073709551615")
    );
    assert_eq!(
        from_value::<OperationId>(json!("18446744073709551615")).expect("decode"),
        last
    );
    for invalid in [
        json!(0),
        json!(1),
        json!("0"),
        json!("01"),
        json!("+1"),
        json!("-1"),
        json!("18446744073709551616"),
    ] {
        assert!(from_value::<OperationId>(invalid).is_err());
    }
}

#[test]
fn sends_require_an_operation_and_private_results_keep_its_owner() {
    let operation = json!({"client":"a".repeat(32),"id":"1"});
    let mut request = json!({"jsonrpc":"2.0","id":"send","method":"session/send",
        "params":{"session":"b".repeat(32),"text":"same text","operation":operation,"policy":"queue"}});
    assert!(from_value::<Request>(request.clone()).is_ok());
    request["params"]
        .as_object_mut()
        .expect("params")
        .remove("operation");
    request["params"]["client_message_id"] = json!("legacy token");
    assert!(from_value::<Request>(request).is_err());
    let result = json!({"jsonrpc":"2.0","method":"operation/updated","params":{
        "operation":operation,"session":"b".repeat(32),"state":{"kind":"uncertain","message":"reply lost",
        "recovery":{"operation":operation,"text":"same text"}}}});
    let parsed: NotificationMessage = from_value(result.clone()).expect("private result");
    assert_eq!(to_value(parsed).expect("encode"), result);
}

#[test]
fn session_creation_requires_the_same_retry_identity_as_other_mutations() {
    let mut create = json!({"jsonrpc":"2.0","id":"create","method":"session/create",
        "params":{"backend":"grok","cwd":"/work","name":null,"operation":{"client":"a".repeat(32),"id":"1"}}});
    assert!(from_value::<Request>(create.clone()).is_ok());
    create["params"]
        .as_object_mut()
        .expect("params")
        .remove("operation");
    assert!(
        from_value::<Request>(create).is_err(),
        "a lost create reply must not permit untracked duplicate sessions"
    );
}

#[test]
fn goal_edits_require_explicit_status_and_reject_native_parameter_fields() {
    let change =
        json!({"objective":"readable goal", "token_budget":"9007199254740995", "status":"paused"});
    assert!(from_value::<carlos_proto::goals::GoalChange>(change.clone()).is_ok());
    for invalid in ["threadId", "origin", "requestMethod"] {
        let mut changed = change.clone();
        changed[invalid] = json!("native-routing");
        assert!(from_value::<carlos_proto::goals::GoalChange>(changed).is_err());
    }
    let mut missing = change;
    missing.as_object_mut().expect("fields").remove("status");
    assert!(from_value::<carlos_proto::goals::GoalChange>(missing).is_err());
    for invalid in ["x".repeat(64), "a".repeat(63), "A".repeat(64)] {
        assert!(from_value::<carlos_proto::ids::GoalRevision>(json!(invalid)).is_err());
    }
}

#[test]
fn queue_changes_reject_native_routes_inputs_and_unoffered_moves() {
    use carlos_proto::queue::QueueChange;
    let edit = json!({"kind":"edit", "entry":"c".repeat(64), "text":"exact draft 🌍"});
    assert!(from_value::<QueueChange>(edit.clone()).is_ok());
    for field in ["input", "threadId", "queuedSubmissionId", "requestMethod"] {
        let mut invalid = edit.clone();
        invalid[field] = json!("native-parameter");
        assert!(from_value::<QueueChange>(invalid).is_err());
    }
    assert!(from_value::<QueueChange>(
        json!({"kind":"move", "entry":"c".repeat(64), "direction":"first"})
    )
    .is_err());
}
