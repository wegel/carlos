//! Keep child reads addressed to their original reader and bounded display page.

use carlos_proto::commands::Command;
use carlos_proto::ids::{TaskActivityId, TaskRevision};
use serde_json::{json, Value};

use super::client::{attached, reply};
use super::session;
use crate::codec::Incoming;
use crate::ReadReply;

fn activity() -> TaskActivityId {
    "b".repeat(64).parse().expect("child reader")
}

fn revision() -> TaskRevision {
    "c".repeat(64).parse().expect("child item revision")
}

fn record() -> Value {
    json!({"id":"d".repeat(64),"turn":"child-turn","item":"child-item",
        "kind":"agentMessage","text":"child 日本語🌍","complete":true})
}

fn page() -> Value {
    json!({"session":session(),"activity":activity(),"revision":revision(),
        "records":[record()],"has_more":false})
}

#[test]
fn child_pages_reject_foreign_targets_repeated_ids_and_oversized_scalars() {
    for kind in [
        "valid", "session", "activity", "revision", "repeat", "cursor", "count", "text", "label",
        "empty",
    ] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::TaskActivityPage {
                    session: session(),
                    activity: activity(),
                    expected: revision(),
                    after: (kind == "cursor")
                        .then(|| "d".repeat(64).parse().expect("original cursor")),
                    limit: Some(if kind == "repeat" { 2 } else { 1 }),
                },
            )
            .expect("bounded child page read");
        let mut value = page();
        match kind {
            "session" => value["session"] = json!("e".repeat(32)),
            "activity" => value["activity"] = json!("e".repeat(64)),
            "revision" => value["revision"] = json!("e".repeat(64)),
            "repeat" | "count" => value["records"] = json!([record(), record()]),
            "text" => value["records"][0]["text"] = json!("x".repeat(32769)),
            "label" => value["records"][0]["turn"] = json!("x".repeat(4097)),
            "empty" => {
                value["records"] = json!([]);
                value["has_more"] = json!(true);
            }
            _ => {}
        }
        let result = client.receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: value,
            },
        );
        assert_eq!(result.is_ok(), kind == "valid", "{kind}");
    }
}

#[test]
fn child_header_stays_on_its_original_task_and_reader() {
    for kind in ["valid", "task", "items", "tail"] {
        let (mut client, connection) = attached();
        let task = "f".repeat(64).parse().expect("captured child task");
        let request = client
            .read(
                connection,
                Command::TaskActivityOpen {
                    session: session(),
                    task,
                    expected: revision(),
                    window: None,
                },
            )
            .expect("explicit child interest");
        let mut header = json!({"session":session(),"activity":activity(),"task":"f".repeat(64),
            "revision":revision(),"change":"3","items":1,"known":true,"reading":false,
            "has_older":false,"closed":false,"error":null,"tail":"live child"});
        match kind {
            "task" => header["task"] = json!("e".repeat(64)),
            "items" => header["items"] = json!(129),
            "tail" => header["tail"] = json!("x".repeat(16385)),
            _ => {}
        }
        let result = client.receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: header,
            },
        );
        assert_eq!(result.is_ok(), kind == "valid", "{kind}");
    }
    let (mut client, connection) = attached();
    let request = client
        .read(
            connection,
            Command::TaskActivityRead {
                session: session(),
                activity: activity(),
                since: None,
            },
        )
        .expect("addressed child header");
    let wrong = json!({"session":session(),"activity":"e".repeat(64),"task":"f".repeat(64),
        "revision":revision(),"change":"3","items":0,"known":true,"reading":false,
        "has_older":false,"closed":false,"error":null,"tail":""});
    assert!(client
        .receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: wrong
            }
        )
        .is_err());
}

#[test]
fn accepted_child_pages_do_not_replace_shared_task_facts() {
    let (mut client, connection) = attached();
    let request = client
        .read(
            connection,
            Command::TaskActivityPage {
                session: session(),
                activity: activity(),
                expected: revision(),
                after: None,
                limit: None,
            },
        )
        .expect("private scalar read");
    let effects = reply(&mut client, &request, page());
    assert!(effects.sessions.is_empty());
    assert!(effects.outgoing.is_empty());
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::TaskActivityPage(page) if page.records[0].text == "child 日本語🌍")
    );
}
