//! Walk complete overview pages and fields through the shared API without a terminal controller.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::task_facts::TaskSnapshot;
use serde_json::{json, Value};

use super::client::{attached, command, greet, notify, reply};
use super::queue_panel::single_read;
use super::task_state::facts;
use super::{entry_id, event, session};
use crate::codec::Incoming;
use crate::{Client, ConnectionId, Effects};

fn opened() -> (Client, ConnectionId, Effects) {
    let (mut client, connection) = attached();
    let _ = client
        .merge_tasks(&TaskSnapshot {
            session: session(),
            seq: Sequence::new(10),
            tasks: facts("c"),
        })
        .expect("accepted task header");
    let opened = client
        .open_task_overview(&session())
        .expect("wanted overview");
    (client, connection, opened)
}

fn row(index: usize, label: &str, bytes: usize) -> Value {
    json!({"id":format!("{index:064x}"),"data":{"kind":"quota",
        "label":{"text":label,"bytes":bytes.to_string(),"complete":label.len()==bytes},
        "used_pct":25,"resets_at":100}})
}

fn page(seq: u64, revision: &str, entries: Vec<Value>, has_more: bool) -> Value {
    json!({"session":session(),"seq":seq.to_string(),"revision":revision.repeat(64),"entries":entries,"has_more":has_more})
}

#[test]
fn all_1153_supported_rows_and_their_long_labels_reach_one_complete_shared_overview() {
    let (mut client, _, mut effects) = opened();
    for start in (0..1153).step_by(32) {
        let request = command(single_read(&effects));
        assert!(matches!(
            request.command,
            Command::TaskOverview {
                limit: Some(32),
                ..
            }
        ));
        let end = (start + 32).min(1153);
        let rows = (start..end)
            .map(|index| {
                row(
                    index,
                    if index == 0 { "start" } else { "quota" },
                    if index == 0 { 10 } else { 5 },
                )
            })
            .collect();
        effects = reply(
            &mut client,
            single_read(&effects),
            page(10, "c", rows, end < 1153),
        );
    }
    assert!(matches!(
        command(single_read(&effects)).command,
        Command::TaskOverviewText { .. }
    ));
    let accepted = reply(
        &mut client,
        single_read(&effects),
        json!({"session":session(),"revision":"c".repeat(64),
        "entry":format!("{:064x}",0),"field":"label","offset":"5","next_offset":"10","text":"🌍!","complete":true}),
    );
    assert!(accepted.replies.is_empty());
    assert!(accepted.outgoing.is_empty());
    let model = client.task_overview(&session()).expect("complete overview");
    assert_eq!(model.entries().len(), 1153);
    assert!(model.has_quota());
    assert!(!model.is_reading());
    let label = &model.entries()[0].data;
    assert!(
        matches!(label,carlos_proto::task_overview::TaskOverviewData::Quota { label, .. } if label.complete && label.text=="start🌍!")
    );
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn one_future_page_waits_for_even_an_empty_numbered_event_without_a_polling_clock() {
    let (mut client, connection, opened) = opened();
    let held = reply(
        &mut client,
        single_read(&opened),
        page(11, "c", vec![row(0, "quota", 5)], false),
    );
    assert!(held.outgoing.is_empty());
    assert!(client
        .task_overview(&session())
        .expect("future reader")
        .entries()
        .is_empty());
    let arrived = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: String::new(),
            },
        )),
    );
    assert!(arrived.outgoing.is_empty());
    assert_eq!(
        client
            .task_overview(&session())
            .expect("numbered page")
            .entries()
            .len(),
        1
    );
}

#[test]
fn a_new_overview_revision_retires_an_old_error_and_its_future_page() {
    let (mut client, connection, opened) = opened();
    let future = reply(
        &mut client,
        single_read(&opened),
        page(12, "c", vec![row(0, "obsolete", 8)], false),
    );
    assert!(future.outgoing.is_empty());
    let changed = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TasksChanged {
                tasks: Some(facts("d")),
            },
        )),
    );
    assert!(
        matches!(command(single_read(&changed)).command,Command::TaskOverview { expected:Some(ref expected), .. } if expected.as_str()=="d".repeat(64))
    );
    let _ = reply(
        &mut client,
        single_read(&changed),
        page(11, "d", vec![row(1, "fresh", 5)], false),
    );
    let stale = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&opened).id()),
                error: RpcError {
                    code: -32001,
                    message: "retired error".into(),
                },
            },
        )
        .expect("retired failure");
    assert!(stale.failures.is_empty());
    let model = client.task_overview(&session()).expect("fresh model");
    assert_eq!(model.entries().len(), 1);
    assert!(model.read_error().is_none());
}

#[test]
fn a_rounded_field_cannot_duplicate_bytes_and_an_error_does_not_refetch_on_root_text() {
    let (mut client, connection, opened) = opened();
    let text = reply(
        &mut client,
        single_read(&opened),
        page(10, "c", vec![row(0, "ab", 4)], false),
    );
    let rejected = reply(
        &mut client,
        single_read(&text),
        json!({"session":session(),"revision":"c".repeat(64),
        "entry":format!("{:064x}",0),"field":"label","offset":"1","next_offset":"4","text":"€","complete":true}),
    );
    assert!(rejected.outgoing.is_empty());
    assert!(client
        .task_overview(&session())
        .expect("kept preview")
        .read_error()
        .is_some());
    let unrelated = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "unrelated".into(),
            },
        )),
    );
    assert!(unrelated.outgoing.is_empty());
    let _ = client.close_task_overview(&session());
    assert_eq!(
        client
            .open_task_overview(&session())
            .expect("explicit retry")
            .outgoing
            .len(),
        1
    );
}

#[test]
fn closing_an_overview_offline_does_not_restore_its_partial_pages_or_interests() {
    let (mut client, connection, opened) = opened();
    let _ = reply(
        &mut client,
        single_read(&opened),
        page(10, "c", vec![row(0, "quota", 5)], true),
    );
    client.disconnect(connection);
    let _ = client.close_task_overview(&session());
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(ready.outgoing.is_empty());
    assert!(client.task_overview(&session()).is_none());
}
