//! Exercise captured detail scopes, exact Unicode assembly and retired point reads without a UI.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::task_facts::TaskSnapshot;
use carlos_proto::tasks::TaskField;
use serde_json::{json, Value};

use super::client::{attached, command, greet, notify, reply};
use super::queue_panel::single_read;
use super::session;
use super::task_state::facts;
use crate::codec::Incoming;
use crate::panels::{TaskDetailTarget, TaskFieldRead, MAX_TASK_DETAIL_BYTES};
use crate::{Client, ConnectionId, Effects, ReadReply};

fn target(older: bool) -> TaskDetailTarget {
    TaskDetailTarget {
        window: older.then(|| "d".repeat(64).parse().expect("captured window")),
        expected: "c".repeat(64).parse().expect("detail revision"),
        task: "b".repeat(64).parse().expect("selected task"),
    }
}

fn detail(preview: &str, bytes: usize) -> Value {
    json!({"session":session(),"revision":target(true).expected,
        "task":{"id":target(true).task,"kind":"agent","label":"selected","label_complete":true,"state":"idle","run":null},
        "properties":[{"field":"report","text":preview,"bytes":bytes.to_string(),"complete":preview.len()==bytes}]})
}

fn text(offset: usize, content: &str, complete: bool) -> Value {
    json!({"session":session(),"revision":target(true).expected,"task":target(true).task,
        "field":"report","offset":offset.to_string(),"next_offset":(offset+content.len()).to_string(),
        "text":content,"complete":complete})
}

fn opened(
    older: bool,
    preview: &str,
    bytes: usize,
    wanted: usize,
) -> (Client, ConnectionId, Effects) {
    let (mut client, connection) = attached();
    let _ = client
        .merge_tasks(&TaskSnapshot {
            session: session(),
            seq: Sequence::new(10),
            tasks: facts(if older { "a" } else { "c" }),
        })
        .expect("latest task header");
    let opened = client
        .open_task_details(&session(), target(older))
        .expect("selected detail");
    let interests = client
        .read_task_fields(
            &session(),
            vec![TaskFieldRead {
                field: TaskField::Report,
                bytes: wanted,
            }],
        )
        .expect("field interest");
    assert!(
        interests.outgoing.is_empty(),
        "choosing fields keeps the initial pending read"
    );
    let effects = reply(&mut client, single_read(&opened), detail(preview, bytes));
    assert!(effects.replies.is_empty());
    (client, connection, effects)
}

#[test]
fn an_older_field_finishes_in_its_original_window_without_replacing_the_latest_header() {
    let (mut client, connection, read) = opened(true, "prefix", 11, MAX_TASK_DETAIL_BYTES);
    assert!(matches!(
        command(single_read(&read)).command,
        Command::TaskText {
            window: Some(_),
            ..
        }
    ));
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(super::event(
            11,
            Event::TasksChanged {
                tasks: Some(facts("e")),
            },
        )),
    );
    let accepted = reply(&mut client, single_read(&read), text(6, "🌍!", true));
    assert!(accepted.outgoing.is_empty());
    let panel = client.task_details(&session()).expect("details");
    assert_eq!(panel.field(TaskField::Report), Some("prefix🌍!"));
    assert!(panel.value().expect("detail").properties[0].complete);
    assert_eq!(
        client.session(&session()).expect("latest session").tasks(),
        Some(&facts("e"))
    );
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn a_latest_reply_cannot_replace_a_new_root_and_a_retired_failure_cannot_poison_a_new_target() {
    let (mut client, connection, read) = opened(false, "a", 5, MAX_TASK_DETAIL_BYTES);
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(super::event(
            11,
            Event::TasksChanged {
                tasks: Some(facts("e")),
            },
        )),
    );
    let obsolete = reply(&mut client, single_read(&read), text(1, "🌍", true));
    assert!(obsolete.outgoing.is_empty());
    assert_eq!(
        client
            .task_details(&session())
            .expect("old preview")
            .field(TaskField::Report),
        Some("a")
    );
    let _ = client.close_task_details(&session());
    let fresh = client
        .open_task_details(&session(), target(true))
        .expect("different captured target");
    let ignored = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&read).id()),
                error: RpcError {
                    code: -32001,
                    message: "obsolete field error".into(),
                },
            },
        )
        .expect("retired error");
    assert!(ignored.failures.is_empty());
    let _ = reply(&mut client, single_read(&fresh), detail("done", 4));
    assert!(client
        .task_details(&session())
        .expect("new reader")
        .read_error()
        .is_none());
}

#[test]
fn a_rounded_unicode_interval_cannot_append_overlapping_bytes() {
    let (mut client, _, read) = opened(true, "ab", 4, MAX_TASK_DETAIL_BYTES);
    // The low-level API permits a UTF-8 floor at a caller-supplied interior offset.
    // The assembler starts at a proven boundary and must reject that same overlap.
    let rejected = reply(&mut client, single_read(&read), text(1, "€", true));
    assert!(rejected.outgoing.is_empty());
    let panel = client.task_details(&session()).expect("failed reader");
    assert_eq!(panel.field(TaskField::Report), Some("ab"));
    assert!(panel.read_error().is_some());
}

#[test]
fn explicit_prefixes_keep_native_incompleteness_and_the_last_single_byte_can_reach_eof() {
    let (mut client, _, read) = opened(true, "aaa", 8, 4);
    assert!(
        read.outgoing.is_empty(),
        "leave room for an unsplit native character"
    );
    let wanted = client
        .read_task_fields(
            &session(),
            vec![TaskFieldRead {
                field: TaskField::Report,
                bytes: 8,
            }],
        )
        .expect("read complete native value");
    let more = reply(&mut client, single_read(&wanted), text(3, "🌍", false));
    assert!(matches!(
        command(single_read(&more)).command,
        Command::TaskText { limit: Some(4), .. }
    ));
    let finished = reply(&mut client, single_read(&more), text(7, "!", true));
    assert!(finished.outgoing.is_empty());
    let panel = client.task_details(&session()).expect("complete value");
    assert_eq!(panel.field(TaskField::Report), Some("aaa🌍!"));
    assert!(panel.value().expect("metadata").properties[0].complete);
}

#[test]
fn disconnect_keeps_captured_fields_readonly_and_offline_close_does_not_restore_them() {
    let (mut client, connection, read) = opened(true, "a", 5, MAX_TASK_DETAIL_BYTES);
    client.disconnect(connection);
    assert!(client
        .task_details(&session())
        .expect("kept preview")
        .is_stale());
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(
        ready.outgoing.is_empty(),
        "do not reuse an expired window on the new socket"
    );
    assert!(reply(&mut client, single_read(&read), text(1, "🌍", true))
        .outgoing
        .is_empty());
    client.disconnect(single_read(&hello).connection());
    let _ = client.close_task_details(&session());
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(ready.outgoing.is_empty());
    assert!(client.task_details(&session()).is_none());
}

#[test]
fn an_unowned_detail_read_stays_available_without_replacing_the_shared_model() {
    let (mut client, connection, _) = opened(true, "owned", 5, MAX_TASK_DETAIL_BYTES);
    let target = target(true);
    let direct = client
        .read(
            connection,
            Command::TaskDetail {
                session: session(),
                window: target.window,
                expected: target.expected,
                task: target.task,
            },
        )
        .expect("explicit independent read");
    let unowned = reply(&mut client, &direct, detail("direct", 6));
    assert!(matches!(unowned.replies[0].reply, ReadReply::TaskDetail(_)));
    assert_eq!(
        client
            .task_details(&session())
            .expect("shared model")
            .field(TaskField::Report),
        Some("owned")
    );
}

#[test]
fn same_value_numbered_task_facts_wake_a_scoped_field_failure_without_a_polling_clock() {
    let (mut client, connection, read) = opened(true, "a", 5, MAX_TASK_DETAIL_BYTES);
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&read).id()),
                error: RpcError {
                    code: -32001,
                    message: "native field failure".into(),
                },
            },
        )
        .expect("owned failure");
    assert!(failed.outgoing.is_empty());
    let retry = notify(
        &mut client,
        connection,
        Notification::SessionEvent(super::event(
            11,
            Event::TasksChanged {
                tasks: Some(facts("a")),
            },
        )),
    );
    let done = reply(&mut client, single_read(&retry), text(1, "🌍", true));
    assert!(done.outgoing.is_empty());
    assert_eq!(
        client
            .task_details(&session())
            .expect("retried complete field")
            .field(TaskField::Report),
        Some("a🌍")
    );
}
