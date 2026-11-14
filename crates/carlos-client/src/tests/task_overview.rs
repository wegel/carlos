//! Check original task overview identities and bounds through the public client API.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::task_overview::TaskOverviewField;
use serde_json::{json, Value};

use super::client::attached;
use super::session;
use crate::codec::Incoming;
use crate::ReadReply;

fn label(text: &str) -> Value {
    json!({"text":text,"bytes":text.len().to_string(),"complete":true})
}

fn entry() -> Value {
    json!({"id":"b".repeat(64),"data":{"kind":"quota","label":label("Window 🌍"),
        "used_pct":25.0,"resets_at":12345678}})
}

#[test]
fn overview_pages_enforce_original_revision_unique_rows_and_named_label_bounds() {
    for kind in [
        "valid",
        "revision",
        "duplicate",
        "over_limit",
        "cursor",
        "label",
        "percentage",
        "empty_more",
    ] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::TaskOverview {
                    session: session(),
                    expected: Some("c".repeat(64).parse().expect("revision")),
                    after: (kind == "cursor").then(|| "b".repeat(64).parse().expect("cursor")),
                    limit: Some(if kind == "duplicate" { 2 } else { 1 }),
                },
            )
            .expect("bounded overview read");
        let mut page = json!({"session":session(),"seq":"10","revision":"c".repeat(64),
            "entries":[entry()],"has_more":false});
        match kind {
            "revision" => page["revision"] = json!("d".repeat(64)),
            "duplicate" => page["entries"] = json!([entry(), entry()]),
            "over_limit" => {
                let mut other = entry();
                other["id"] = json!("d".repeat(64));
                page["entries"] = json!([entry(), other]);
            }
            "label" => page["entries"][0]["data"]["label"]["bytes"] = json!("1"),
            "percentage" => page["entries"][0]["data"]["used_pct"] = json!(101),
            "empty_more" => {
                page["entries"] = json!([]);
                page["has_more"] = json!(true);
            }
            _ => {}
        }
        let result = client.receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: page,
            },
        );
        if kind == "valid" {
            assert!(matches!(&result.expect("valid page").replies[0].reply,
                ReadReply::TaskOverview(page) if page.entries.len() == 1));
        } else {
            assert!(result.is_err(), "{kind}");
        }
    }
}

#[test]
fn overview_text_matches_its_original_entry_field_revision_and_utf8_interval() {
    for kind in [
        "emoji",
        "eof",
        "wrong_entry",
        "wrong_field",
        "wrong_revision",
        "ascii",
        "shifted_empty",
        "too_long",
    ] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::TaskOverviewText {
                    session: session(),
                    expected: "c".repeat(64).parse().expect("revision"),
                    entry: "b".repeat(64).parse().expect("entry"),
                    field: TaskOverviewField::Label,
                    offset: ByteOffset::new(if kind == "eof" { 8 } else { 5 }),
                    limit: Some(4),
                },
            )
            .expect("named overview read");
        let value = text_reply(kind);
        let result = client.receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: value,
            },
        );
        if kind == "emoji" || kind == "eof" {
            assert!(matches!(
                &result.expect("valid interval").replies[0].reply,
                ReadReply::TaskOverviewText(_)
            ));
        } else {
            assert!(result.is_err(), "{kind}");
        }
    }
}

fn text_reply(kind: &str) -> Value {
    let mut value = json!({"session":session(),"revision":"c".repeat(64),"entry":"b".repeat(64),
        "field":"label","offset":"4","next_offset":"8","text":"🌍","complete":true});
    match kind {
        "wrong_entry" => value["entry"] = json!("d".repeat(64)),
        "wrong_revision" => value["revision"] = json!("d".repeat(64)),
        "wrong_field" => value["field"] = json!("message"),
        "ascii" => {
            value["text"] = json!("a");
            value["next_offset"] = json!("5");
        }
        "eof" | "shifted_empty" => {
            value["text"] = json!("");
            let start = if kind == "eof" { "8" } else { "4" };
            value["offset"] = json!(start);
            value["next_offset"] = json!(start);
        }
        "too_long" => {
            value["offset"] = json!("5");
            value["next_offset"] = json!("10");
            value["text"] = json!("12345");
        }
        _ => {}
    }
    value
}
