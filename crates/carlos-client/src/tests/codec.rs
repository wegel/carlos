//! Reject ambiguous envelopes and frame partial Unicode messages without rescanning old bytes.

use carlos_proto::commands::{Command, RequestId};
use serde_json::json;

use crate::codec::{self, Framer, Incoming};

#[test]
fn split_utf8_and_several_lines_decode_once_in_order() {
    let mut framer = Framer::default();
    let message = json!({"jsonrpc":"2.0","id":"first","result":"🌍"}).to_string() + "\n";
    for byte in message.as_bytes() {
        framer.push(&[*byte]).expect("bounded byte");
    }
    let Some(Incoming::Success { result, .. }) = framer.next_message().expect("decode") else {
        panic!("reply");
    };
    assert_eq!(result, json!("🌍"));
    assert!(framer.next_message().expect("empty").is_none());
    assert!(!framer.has_partial());
    for _ in 0..100 {
        framer.push(message.as_bytes()).expect("bounded stream");
        assert!(framer.next_message().expect("next").is_some());
    }
    framer.push(b"{\"jsonrpc\"").expect("partial");
    assert!(framer.next_message().expect("wait").is_none());
    assert!(framer.has_partial());
}

#[test]
fn ambiguous_version_unknown_method_and_unbounded_ids_fail() {
    for value in [
        json!({"jsonrpc":"1.0","id":1,"result":null}),
        json!({"jsonrpc":"2.0","id":1,"result":null,"error":{}}),
        json!({"jsonrpc":"2.0","id":null,"result":null}),
        json!({"jsonrpc":"2.0","method":"future/event","params":{}}),
        json!({"jsonrpc":"2.0","method":"sessions/changed","id":1,"params":{}}),
        json!({"jsonrpc":"2.0","id":"x".repeat(257),"result":null}),
    ] {
        assert!(
            codec::decode(value.to_string().as_bytes()).is_err(),
            "{value}"
        );
    }
}

#[test]
fn requests_keep_their_rpc_version_and_private_failure_shape_is_explicit() {
    let request = codec::request(RequestId::String("watch".into()), Command::Watch {});
    let bytes = codec::encode(&request).expect("encode");
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert!(matches!(
        codec::decode(
            br#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"bad request"}}"#
        )
        .expect("error"),
        Incoming::Failure { id: None, .. }
    ));
}

#[test]
fn escaped_request_growth_and_stream_overflow_fail_before_retaining_oversized_bytes() {
    let huge = codec::request(
        RequestId::Integer(1),
        Command::Hello(carlos_proto::commands::Hello {
            protocol: 1,
            client: "\0".repeat(1024 * 1024),
            token: Default::default(),
            resume: None,
            resume_token: None,
            instance: None,
        }),
    );
    assert!(matches!(
        codec::encode(&huge),
        Err(crate::ClientError::Capacity(_))
    ));
    let invalid_id = codec::request(RequestId::String("i".repeat(257)), Command::Watch {});
    assert!(codec::encode(&invalid_id).is_err());
    let mut framer = Framer::default();
    framer.push(b"{\"jsonrpc\":").expect("partial");
    assert!(framer.push(&vec![b'x'; codec::MAX_FRAME_BYTES]).is_err());
    framer
        .push(b"\"2.0\",\"id\":1,\"result\":null}\n")
        .expect("recover after overflow");
    assert!(matches!(
        framer.next_message().expect("original frame preserved"),
        Some(Incoming::Success { .. })
    ));
}
