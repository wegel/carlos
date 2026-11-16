//! Preserve candidate windows, exact close rights and readonly older rows across reconnect.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::tasks::TaskField;
use serde_json::{json, Value};

use super::client::{command, greet, reply};
use super::queue_panel::single_read;
use super::session;
use super::task_panel::{header, id, page, read, settled};
use crate::codec::Incoming;
use crate::panels::{TaskDetailTarget, TaskFieldRead};
use crate::{Client, Effects, ReadReply};

fn window(known: bool) -> Value {
    json!({"session":session(),"seq":"10","window":"d".repeat(64),"revision":"d".repeat(64),
        "known":known,"reading":!known,"rows":if known { 2 } else { 0 },"older_available":true,"read_error":null})
}

fn candidate(client: &mut Client) -> Effects {
    let opened = client
        .open_older_task_window(&session())
        .expect("explicit older window");
    let waiting = reply(client, read(&opened, "task/window/open"), window(false));
    assert_eq!(
        client
            .task_panel(&session())
            .expect("current page kept")
            .selected_id(),
        Some(&id(0))
    );
    reply(client, read(&waiting, "task/window/read"), window(true))
}

fn accept_older(client: &mut Client) {
    let first = candidate(client);
    let accepted = reply(client, read(&first, "task/list"), page(10, "d", 8, 10, 2));
    assert!(matches!(
        command(read(&accepted, "task/unsubscribe")).command,
        Command::TaskUnsubscribe { .. }
    ));
}

#[test]
fn a_complete_candidate_replaces_the_page_but_never_the_sessions_latest_header() {
    let (mut client, _) = settled();
    let first = candidate(&mut client);
    assert!(client
        .task_panel(&session())
        .expect("candidate not yet visible")
        .window()
        .is_none());
    assert!(matches!(
        command(read(&first, "task/list")).command,
        Command::TaskList {
            window: Some(_),
            ..
        }
    ));
    reply(
        &mut client,
        read(&first, "task/list"),
        page(10, "d", 8, 10, 2),
    );
    let model = client
        .task_panel(&session())
        .expect("captured older window");
    assert_eq!(model.selected_id(), Some(&id(8)));
    assert_eq!(model.window().expect("accepted native scope").rows, 2);
    assert_eq!(
        client
            .session(&session())
            .expect("latest facts")
            .tasks()
            .expect("single header")
            .revision
            .as_str(),
        "c".repeat(64)
    );
    assert!(client.task_panel_is_current(&session()));
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn a_failed_older_read_keeps_selection_and_closes_only_its_original_candidate() {
    let (mut client, connection) = settled();
    let opened = client
        .open_older_task_window(&session())
        .expect("older window");
    let waiting = reply(
        &mut client,
        read(&opened, "task/window/open"),
        window(false),
    );
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(read(&waiting, "task/window/read").id()),
                error: RpcError {
                    code: -32001,
                    message: "native older failure".into(),
                },
            },
        )
        .expect("owned candidate failure");
    assert!(
        matches!(command(read(&failed,"task/window/close")).command,Command::TaskWindowClose { window:Some(ref window), .. } if window.as_str()=="d".repeat(64))
    );
    let model = client.task_panel(&session()).expect("previous page");
    assert_eq!(model.selected_id(), Some(&id(0)));
    assert!(model.window().is_none());
    assert!(model
        .read_error()
        .expect("candidate error")
        .contains("native older failure"));
    client
        .move_task_panel(&session(), 1)
        .expect("current page still navigates");
    assert_eq!(
        client
            .task_panel(&session())
            .expect("next selected row")
            .selected_id(),
        Some(&id(1))
    );
    assert!(client
        .task_panel(&session())
        .expect("explicit arrow clears read error")
        .read_error()
        .is_none());
}

#[test]
fn a_retired_open_reply_closes_its_exact_window_without_replacing_a_reopened_latest_page() {
    let (mut client, _) = settled();
    let old = client
        .open_older_task_window(&session())
        .expect("allocating older open");
    client.close_task_panel(&session());
    let fresh = client
        .open_task_panel(&session(), None)
        .expect("reopened task panel");
    reply(
        &mut client,
        read(&fresh, "task/subscribe"),
        header(10, "c", 2),
    );
    let late = reply(&mut client, read(&old, "task/window/open"), window(true));
    assert!(late.replies.is_empty());
    assert!(matches!(
        command(read(&late, "task/window/close")).command,
        Command::TaskWindowClose {
            window: Some(_),
            ..
        }
    ));
    let model = client.task_panel(&session()).expect("latest page survives");
    assert!(model.window().is_none());
    assert_eq!(model.selected_id(), Some(&id(0)));
    assert!(model.read_error().is_none());
}

#[test]
fn an_unowned_window_reply_keeps_its_own_scope_when_the_panel_closes() {
    let (mut client, connection) = settled();
    accept_older(&mut client);
    let direct = client
        .read(
            connection,
            Command::TaskWindowOpen {
                session: session(),
                expected: "c".repeat(64).parse().expect("latest revision"),
                after: None,
            },
        )
        .expect("independent native capture");
    let mut foreign = window(true);
    foreign["window"] = json!("f".repeat(64));
    foreign["revision"] = json!("f".repeat(64));
    let unowned = reply(&mut client, &direct, foreign);
    assert!(matches!(unowned.replies[0].reply, ReadReply::TaskWindow(_)));
    let closed = client.close_task_panel(&session());
    assert!(
        matches!(command(read(&closed,"task/window/close")).command,Command::TaskWindowClose { window:Some(ref window), .. } if window.as_str()=="d".repeat(64))
    );
    assert!(client.close_task_panel(&session()).outgoing.is_empty());
}

#[test]
fn an_older_window_keeps_readonly_rows_after_eof_and_only_explicit_refresh_restores_latest_interest(
) {
    let (mut client, connection) = settled();
    accept_older(&mut client);
    client.disconnect(connection);
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(ready.outgoing.is_empty());
    assert!(!client.task_panel_is_current(&session()));
    let model = client.task_panel(&session()).expect("readonly capture");
    assert!(model.window_is_stale());
    assert_eq!(model.selected_id(), Some(&id(8)));
    let refresh = client
        .refresh_task_panel(&session())
        .expect("explicit latest");
    assert!(matches!(
        command(read(&refresh, "task/subscribe")).command,
        Command::TaskSubscribe { .. }
    ));
    let next = reply(
        &mut client,
        read(&refresh, "task/subscribe"),
        header(10, "c", 2),
    );
    reply(
        &mut client,
        read(&next, "task/list"),
        page(10, "c", 0, 2, 2),
    );
    let model = client.task_panel(&session()).expect("latest page");
    assert!(model.window().is_none());
    assert_eq!(
        model.selected_id(),
        Some(&id(8)),
        "explicit latest cannot select another task for a stop"
    );
    assert!(model.selected().is_none());
}

#[test]
fn closing_a_captured_window_keeps_accepted_details_readonly_and_retires_its_pending_text() {
    let (mut client, _) = settled();
    accept_older(&mut client);
    let target = TaskDetailTarget {
        window: Some("d".repeat(64).parse().expect("owned window")),
        expected: "d".repeat(64).parse().expect("captured revision"),
        task: id(8),
    };
    let opened = client
        .open_task_details(&session(), target)
        .expect("selected detail");
    reply(
        &mut client,
        single_read(&opened),
        json!({"session":session(), "revision":"d".repeat(64),
        "task":page(10,"d",8,9,2)["tasks"][0],
        "properties":[{"field":"report","text":"kept","bytes":"8","complete":false}]}),
    );
    let pending = client
        .read_task_fields(
            &session(),
            vec![TaskFieldRead {
                field: TaskField::Report,
                bytes: 8,
            }],
        )
        .expect("long report");
    let closed = client.close_task_panel(&session());
    read(&closed, "task/window/close");
    let detail = client
        .task_details(&session())
        .expect("readonly accepted details");
    assert!(detail.is_stale());
    assert!(!detail.is_reading());
    assert_eq!(detail.field(TaskField::Report), Some("kept"));
    let late = reply(
        &mut client,
        single_read(&pending),
        json!({"session":session(),
        "revision":"d".repeat(64),"task":id(8),"field":"report",
        "offset":"4","next_offset":"8","text":"late","complete":true}),
    );
    assert!(late.outgoing.is_empty() && late.replies.is_empty());
    assert_eq!(
        client
            .task_details(&session())
            .expect("retained")
            .field(TaskField::Report),
        Some("kept")
    );
    let reopened = client
        .open_task_panel(&session(), None)
        .expect("explicit reopen");
    assert!(
        reopened.outgoing.is_empty(),
        "do not read the retired native window"
    );
}
