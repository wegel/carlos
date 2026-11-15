//! Preserve task read targets and private stop ownership through the public shared client.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::{Command, Request};
use carlos_proto::ids::{TaskId, TaskReadId, TaskRevision, TaskRun};
use carlos_proto::submissions::OperationState;
use carlos_proto::tasks::TaskField;
use serde_json::{json, Value};

use super::client::{attached, reply};
use super::session;
use crate::codec::Incoming;
use crate::{ClientError, ReadReply};

fn task() -> TaskId {
    "b".repeat(64).parse().expect("task address")
}

fn revision() -> TaskRevision {
    "c".repeat(64).parse().expect("metadata revision")
}

fn run() -> TaskRun {
    "d".repeat(64).parse().expect("native run precondition")
}

fn task_row() -> Value {
    json!({"id":task(), "kind":"agent", "label":"Alpha 日本語", "label_complete":true,
        "state":"active", "run":run()})
}

#[test]
fn task_list_and_details_remain_explicit_typed_reads() {
    let (mut client, connection) = attached();
    let list = client
        .read(
            connection,
            Command::TaskList {
                session: session(),
                expected: Some(revision()),
                after: None,
                limit: Some(1),
                refresh: false,
            },
        )
        .expect("typed list read");
    let effects = reply(
        &mut client,
        &list,
        json!({"session":session(), "seq":"10", "revision":revision(),
        "known":true, "reading":false, "closed":false, "tasks":[task_row()],
        "has_more":false, "native_older_available":false, "read_error":null}),
    );
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::TaskPage(page) if page.tasks[0].id == task())
    );
    let details = client
        .read(
            connection,
            Command::TaskDetail {
                session: session(),
                expected: revision(),
                task: task(),
            },
        )
        .expect("typed selected detail");
    let effects = reply(
        &mut client,
        &details,
        json!({"session":session(), "revision":revision(),
        "task":task_row(), "properties":[{"field":"role", "text":"reviewer", "bytes":"8", "complete":true}]}),
    );
    assert!(
        matches!(&effects.replies[0].reply, ReadReply::TaskDetail(detail) if detail.task.id == task())
    );
    assert!(effects.outgoing.is_empty());
}

#[test]
fn task_pages_reject_foreign_revisions_repeated_rows_and_excess_rows() {
    for kind in [
        "foreign_session",
        "changed_revision",
        "repeated",
        "too_many",
    ] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::TaskList {
                    session: session(),
                    expected: Some(revision()),
                    after: None,
                    limit: Some(if kind == "repeated" { 2 } else { 1 }),
                    refresh: false,
                },
            )
            .expect("bounded task read");
        let mut page = json!({"session":session(), "seq":"10", "revision":revision(),
            "known":true, "reading":false, "closed":false, "tasks":[task_row()],
            "has_more":false, "native_older_available":false, "read_error":null});
        match kind {
            "foreign_session" => page["session"] = json!("e".repeat(32)),
            "changed_revision" => page["revision"] = json!("f".repeat(64)),
            "repeated" => page["tasks"] = json!([task_row(), task_row()]),
            _ => {
                let mut other = task_row();
                other["id"] = json!("e".repeat(64));
                page["tasks"] = json!([task_row(), other]);
            }
        }
        assert!(
            client
                .receive(
                    connection,
                    Incoming::Success {
                        id: request.id(),
                        result: page
                    }
                )
                .is_err(),
            "{kind}"
        );
    }
}

#[test]
fn named_task_text_checks_the_original_field_revision_and_utf8_range() {
    for kind in [
        "valid",
        "wrong_field",
        "wrong_start",
        "wrong_end",
        "no_progress",
        "backward",
        "ascii_rounding",
        "empty_shifted",
    ] {
        let (mut client, connection) = attached();
        let request = client
            .read(
                connection,
                Command::TaskText {
                    session: session(),
                    expected: revision(),
                    task: task(),
                    field: TaskField::Report,
                    offset: ByteOffset::new(5),
                    limit: Some(64),
                },
            )
            .expect("named source read");
        let text = task_text_reply(kind);
        let result = client.receive(
            connection,
            Incoming::Success {
                id: request.id(),
                result: text,
            },
        );
        if kind == "valid" {
            assert!(
                matches!(&result.expect("valid Unicode range").replies[0].reply, ReadReply::TaskText(text) if text.text == "🌍")
            );
        } else {
            assert!(
                matches!(result, Err(ClientError::InvalidPayload(_))),
                "{kind}"
            );
        }
    }
}

fn task_text_reply(kind: &str) -> Value {
    let mut text = json!({"session":session(), "revision":revision(), "task":task(), "field":"report",
        "offset":"4", "next_offset":"8", "complete":true, "text":"🌍"});
    match kind {
        "wrong_field" => text["field"] = json!("role"),
        "wrong_start" => text["offset"] = json!("0"),
        "wrong_end" => text["next_offset"] = json!("9"),
        "no_progress" | "empty_shifted" => {
            text["text"] = json!("");
            text["next_offset"] = json!("4");
            text["complete"] = json!(kind == "empty_shifted");
        }
        "backward" | "ascii_rounding" => {
            text["text"] = json!("a");
            text["offset"] = json!(if kind == "backward" { "2" } else { "4" });
            text["next_offset"] = json!(if kind == "backward" { "3" } else { "5" });
            text["complete"] = json!(false);
        }
        _ => {}
    }
    text
}

#[test]
fn task_run_continuations_cannot_follow_a_replaced_native_read() {
    let (mut client, connection) = attached();
    let read: TaskReadId = "e".repeat(64).parse().expect("original read");
    let request = client
        .read(
            connection,
            Command::TaskRun {
                session: session(),
                task: task(),
                read: Some(read),
                refresh: false,
            },
        )
        .expect("continue original run read");
    let result = client.receive(
        connection,
        Incoming::Success {
            id: request.id(),
            result: json!({
        "session":session(), "task":task(), "read":"f".repeat(64), "run":run(), "known":true,
        "native_state":"inProgress", "reading":false, "closed":false, "error":null}),
        },
    );
    assert!(matches!(result, Err(ClientError::InvalidPayload(_))));
}

#[test]
fn task_stops_keep_private_operation_ownership_without_creating_prompt_text() {
    let (mut client, connection) = attached();
    let request = client
        .submit(connection, |operation| Command::TaskStop {
            session: session(),
            operation,
            task: task(),
            expected_run: run(),
        })
        .expect("typed task stop");
    let command: Request = serde_json::from_slice(request.bytes()).expect("encoded task stop");
    let Command::TaskStop { operation, .. } = command.command else {
        panic!("task stop command");
    };
    assert!(client
        .operations()
        .get(&operation)
        .expect("private result")
        .text()
        .is_none());
    let effects = reply(
        &mut client,
        &request,
        json!({"operation":operation, "session":session(),
        "state":{"kind":"accepted", "entry":null}}),
    );
    assert_eq!(effects.operations, vec![operation.clone()]);
    assert!(matches!(
        client
            .operations()
            .get(&operation)
            .expect("accepted stop")
            .state,
        OperationState::Accepted {
            entry: None,
            notice: None
        }
    ));
}
