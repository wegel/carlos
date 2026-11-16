//! Drive complete task windows, future facts and selected identities through the public client API.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::TaskId;
use carlos_proto::task_facts::TaskFacts;
use serde_json::{json, Value};

use super::client::{attached, command, greet, notify, reply};
use super::queue_panel::single_read;
use super::task_state::facts;
use super::{entry_id, event, session};
use crate::codec::Incoming;
use crate::{Client, ConnectionId, Effects, Outbound, ReadReply};

pub(super) fn id(index: usize) -> TaskId {
    format!("{index:064x}").parse().expect("opaque task")
}

pub(super) fn task_facts(version: &str, count: usize) -> TaskFacts {
    let mut header = facts(version);
    header.counts.tasks = count as u32;
    header
}

pub(super) fn header(seq: u64, version: &str, count: usize) -> Value {
    json!({"session":session(),"seq":seq.to_string(),"tasks":task_facts(version,count)})
}

pub(super) fn page(seq: u64, version: &str, start: usize, end: usize, count: usize) -> Value {
    let rows:Vec<_> = (start..end).map(|index| json!({"id":id(index),"kind":"task","label":format!("task {index}"),"label_complete":true,"state":"running","run":null})).collect();
    json!({"session":session(),"seq":seq.to_string(),"revision":version.repeat(64),"known":true,"reading":false,"closed":false,
        "tasks":rows,"has_more":end<count,"native_older_available":false,"read_error":null})
}

pub(super) fn read<'a>(effects: &'a Effects, method: &str) -> &'a Outbound {
    effects
        .outgoing
        .iter()
        .find(|outgoing| {
            serde_json::from_slice::<Value>(outgoing.bytes()).expect("typed outgoing")["method"]
                == method
        })
        .unwrap_or_else(|| panic!("missing {method}"))
}

pub(super) fn settled() -> (Client, ConnectionId) {
    let (mut client, connection) = attached();
    let opened = client
        .open_task_panel(&session(), None)
        .expect("task panel");
    let next = reply(
        &mut client,
        read(&opened, "task/subscribe"),
        header(10, "c", 2),
    );
    reply(
        &mut client,
        read(&next, "task/list"),
        page(10, "c", 0, 2, 2),
    );
    (client, connection)
}

#[test]
fn the_shared_reader_walks_all_4352_supported_rows_to_the_original_selected_identity() {
    let (mut client, _) = attached();
    let opened = client
        .open_task_panel(&session(), Some(id(4351)))
        .expect("last selected task");
    let mut effects = reply(&mut client, single_read(&opened), header(10, "c", 4352));
    for start in (0..4352).step_by(32) {
        assert!(matches!(
            command(read(&effects, "task/list")).command,
            Command::TaskList {
                limit: Some(32),
                ..
            }
        ));
        effects = reply(
            &mut client,
            read(&effects, "task/list"),
            page(10, "c", start, start + 32, 4352),
        );
        assert!(effects.replies.is_empty());
    }
    assert!(effects.outgoing.is_empty());
    let model = client.task_panel(&session()).expect("settled last page");
    assert_eq!(model.selected().expect("selected row").id, id(4351));
    assert_eq!(model.pages().position(), Some(4351));
    assert_eq!(model.pages().entries().len(), 32);
    assert_eq!(model.count(), 4352);
    assert!(client.task_panel_is_current(&session()));
}

#[test]
fn both_an_owned_header_and_page_wait_for_numbered_facts_without_copying_session_authority() {
    let (mut client, connection) = attached();
    let opened = client
        .open_task_panel(&session(), None)
        .expect("task panel");
    let held = reply(&mut client, single_read(&opened), header(11, "c", 2));
    assert!(held.outgoing.is_empty());
    assert!(client
        .session(&session())
        .expect("one session")
        .tasks()
        .is_none());
    let ready = notify(
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
    let page_held = reply(
        &mut client,
        read(&ready, "task/list"),
        page(12, "c", 0, 2, 2),
    );
    assert!(page_held.outgoing.is_empty());
    assert!(client
        .task_panel(&session())
        .expect("waiting page")
        .selected()
        .is_none());
    let settled = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            12,
            Event::TextAppended {
                entry: entry_id(10),
                text: String::new(),
            },
        )),
    );
    assert!(settled.outgoing.is_empty());
    assert_eq!(
        client
            .task_panel(&session())
            .expect("numbered page")
            .selected_id(),
        Some(&id(0))
    );
    assert_eq!(
        client.session(&session()).expect("one header").tasks(),
        Some(&task_facts("c", 2))
    );
}

#[test]
fn a_removed_selected_task_stays_unavailable_until_an_explicit_arrow_chooses_another_identity() {
    let (mut client, connection) = settled();
    client
        .move_task_panel(&session(), 1)
        .expect("select second task");
    let changed = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TasksChanged {
                tasks: Some(task_facts("d", 1)),
            },
        )),
    );
    reply(
        &mut client,
        read(&changed, "task/list"),
        page(11, "d", 0, 1, 1),
    );
    let panel = client
        .task_panel(&session())
        .expect("removed selected task");
    assert_eq!(panel.selected_id(), Some(&id(1)));
    assert!(panel.selected().is_none());
    client
        .move_task_panel(&session(), 1)
        .expect("explicit replacement selection");
    assert_eq!(
        client
            .task_panel(&session())
            .expect("new selection")
            .selected_id(),
        Some(&id(0))
    );
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn a_page_failure_ignores_root_text_but_retries_after_even_same_value_numbered_task_facts() {
    let (mut client, connection) = attached();
    let opened = client
        .open_task_panel(&session(), None)
        .expect("task reader");
    let next = reply(&mut client, single_read(&opened), header(10, "c", 2));
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(read(&next, "task/list").id()),
                error: RpcError {
                    code: -32001,
                    message: "native page failure".into(),
                },
            },
        )
        .expect("owned page error");
    assert!(failed.outgoing.is_empty());
    let unrelated = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "root text".into(),
            },
        )),
    );
    assert!(unrelated.outgoing.is_empty());
    let retry = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            12,
            Event::TasksChanged {
                tasks: Some(task_facts("c", 2)),
            },
        )),
    );
    reply(
        &mut client,
        read(&retry, "task/list"),
        page(12, "c", 0, 2, 2),
    );
    assert!(client
        .task_panel(&session())
        .expect("retried page")
        .read_error()
        .is_none());
}

#[test]
fn closing_offline_does_not_restore_a_task_watch_or_consume_an_unowned_page() {
    let (mut client, connection) = settled();
    let direct = client
        .read(
            connection,
            Command::TaskList {
                session: session(),
                window: None,
                expected: Some("c".repeat(64).parse().expect("revision")),
                after: None,
                limit: Some(32),
                refresh: false,
            },
        )
        .expect("explicit independent read");
    assert!(matches!(
        reply(&mut client, &direct, page(10, "c", 0, 2, 2)).replies[0].reply,
        ReadReply::TaskPage(_)
    ));
    client.disconnect(connection);
    assert!(client.close_task_panel(&session()).outgoing.is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    assert!(reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]})
    )
    .outgoing
    .is_empty());
    assert!(!client.task_panel_is_current(&session()));
    assert_eq!(
        client
            .task_panel(&session())
            .expect("retained selection")
            .selected_id(),
        Some(&id(0))
    );
}

#[test]
fn unsupported_counts_do_not_overflow_or_start_a_partial_native_view() {
    let (mut client, _) = attached();
    let opened = client
        .open_task_panel(&session(), None)
        .expect("task reader");
    let mut invalid = header(10, "c", 0);
    for field in ["agents", "processes", "tasks"] {
        invalid["tasks"]["counts"][field] = json!(u32::MAX);
    }
    let refused = reply(&mut client, single_read(&opened), invalid);
    assert!(refused.outgoing.is_empty());
    assert!(client
        .task_panel(&session())
        .expect("explicit count error")
        .read_error()
        .is_some());
    assert!(!client.task_panel_is_current(&session()));
}

#[test]
fn ordered_arrows_wait_for_the_original_identity_and_use_the_refreshed_counts() {
    let (mut client, connection) = settled();
    client
        .move_task_panel(&session(), 1)
        .expect("select original second row");
    let gap = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TasksChanged {
                tasks: Some(task_facts("d", 3)),
            },
        )),
    );
    let mut latest = Effects::default();
    for direction in [1, -1, 1] {
        latest = client
            .move_task_panel(&session(), direction)
            .expect("deferred ordered move");
        assert_eq!(
            client
                .task_panel(&session())
                .expect("original target kept")
                .selected_id(),
            Some(&id(1))
        );
    }
    let mut rows = page(11, "d", 0, 3, 3);
    rows["tasks"].as_array_mut().expect("task rows").reverse();
    reply(&mut client, read(&latest, "task/list"), rows.clone());
    assert_eq!(
        client
            .task_panel(&session())
            .expect("ordered target settles")
            .selected_id(),
        Some(&id(0))
    );
    let ignored = reply(&mut client, read(&gap, "task/list"), rows);
    assert!(ignored.replies.is_empty());
    assert!(ignored.outgoing.is_empty());
}

#[test]
fn an_explicit_new_target_retires_an_old_page_and_its_unsettled_arrow_moves() {
    let (mut client, _) = attached();
    let opened = client
        .open_task_panel(&session(), Some(id(0)))
        .expect("first target");
    let first = reply(&mut client, single_read(&opened), header(10, "c", 65));
    let old = client
        .move_task_panel(&session(), 1)
        .expect("deferred move from old target");
    let mut current = client
        .open_task_panel(&session(), Some(id(64)))
        .expect("explicit different target");
    for start in (0..65).step_by(32) {
        current = reply(
            &mut client,
            read(&current, "task/list"),
            page(10, "c", start, (start + 32).min(65), 65),
        );
    }
    assert_eq!(
        client
            .task_panel(&session())
            .expect("explicit target settles")
            .selected_id(),
        Some(&id(64))
    );
    for effects in [first, old] {
        let ignored = reply(
            &mut client,
            read(&effects, "task/list"),
            page(10, "c", 0, 32, 65),
        );
        assert!(ignored.replies.is_empty());
        assert!(ignored.outgoing.is_empty());
    }
}
