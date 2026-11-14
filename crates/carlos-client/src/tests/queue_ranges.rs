//! Reject malformed named queue intervals before a platform attempts to assemble an editor.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use serde_json::{json, Value};

use super::client::attached;
use super::session;
use crate::codec::Incoming;
use crate::ReadReply;

#[test]
fn queue_text_rejects_backward_ascii_shifted_eof_stalls_and_request_limit_overruns() {
    for kind in [
        "emoji",
        "eof",
        "backward",
        "ascii_rounding",
        "shifted_eof",
        "stall",
        "over_limit",
        "other_entry",
        "other_revision",
    ] {
        let (mut client, connection) = attached();
        let (offset, text) = queue_reply(kind);
        let request = client
            .read(
                connection,
                Command::QueueText {
                    session: session(),
                    expected: "c".repeat(64).parse().expect("queue revision"),
                    entry: "b".repeat(64).parse().expect("queue entry"),
                    offset: ByteOffset::new(offset),
                    limit: Some(4),
                },
            )
            .expect("explicit named queue source");
        let result = client.receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: text,
            },
        );
        if kind == "emoji" || kind == "eof" {
            let result = result.expect("valid native Unicode interval");
            assert!(matches!(&result.replies[0].reply,ReadReply::QueueText(text)
                if text.complete && (text.text == "🌍" || text.text.is_empty())));
        } else {
            assert!(result.is_err(), "{kind}");
        }
    }
}

fn queue_reply(kind: &str) -> (u64, Value) {
    let mut offset = 5;
    let mut value = json!({"session":session(),"revision":"c".repeat(64),"entry":"b".repeat(64),
        "offset":"4","next_offset":"8","complete":true,"text":"🌍"});
    match kind {
        "eof" | "shifted_eof" | "stall" => {
            offset = if kind == "eof" { 8 } else { 5 };
            let start = if kind == "shifted_eof" { 4 } else { offset };
            value["offset"] = json!(start.to_string());
            value["next_offset"] = json!(start.to_string());
            value["text"] = json!("");
            value["complete"] = json!(kind != "stall");
        }
        "backward" | "ascii_rounding" => {
            let start = if kind == "backward" { 2 } else { 4 };
            value["offset"] = json!(start.to_string());
            value["next_offset"] = json!((start + 1).to_string());
            value["text"] = json!("a");
            value["complete"] = json!(false);
        }
        "over_limit" => {
            value["offset"] = json!("5");
            value["next_offset"] = json!("10");
            value["text"] = json!("12345");
        }
        "other_entry" => value["entry"] = json!("e".repeat(64)),
        "other_revision" => value["revision"] = json!("e".repeat(64)),
        _ => {}
    }
    (offset, value)
}
