//! Preserve native child identities, readonly displays and exact close rights through shared APIs.

use carlos_proto::commands::{Command, RpcError};
use serde_json::{json, Value};

use super::client::{attached, command, greet, reply};
use super::session;
use crate::codec::Incoming;
use crate::panels::TaskDetailTarget;
use crate::{Client, ConnectionId, Effects, Outbound, ReadReply};

fn target() -> TaskDetailTarget {
    TaskDetailTarget {
        window: Some("d".repeat(64).parse().expect("captured window")),
        expected: "c".repeat(64).parse().expect("captured revision"),
        task: "b".repeat(64).parse().expect("captured task"),
    }
}

fn opened() -> (Client, ConnectionId, Effects) {
    let (mut client, connection) = attached();
    let opened = client
        .open_task_activity(&session(), target())
        .expect("explicit child open");
    assert!(matches!(
        command(read(&opened, "task/activity/open")).command,
        Command::TaskActivityOpen {
            window: Some(_),
            ..
        }
    ));
    (client, connection, opened)
}

fn read<'a>(effects: &'a Effects, method: &str) -> &'a Outbound {
    effects
        .outgoing
        .iter()
        .find(|outgoing| {
            let value: Value = serde_json::from_slice(outgoing.bytes()).expect("typed request");
            value["method"] == method
        })
        .unwrap_or_else(|| panic!("missing {method}"))
}

fn header(activity: &str, revision: &str, change: u64) -> Value {
    json!({"session":session(),"activity":activity.repeat(64),"task":target().task,
        "revision":revision.repeat(64),"change":change.to_string(),"items":2,"known":true,"reading":false,
        "has_older":true,"closed":false,"error":null,"tail":"live tail"})
}

fn page(activity: &str, revision: &str, start: usize, more: bool) -> Value {
    json!({"session":session(),"activity":activity.repeat(64),"revision":revision.repeat(64),
        "records":[{"id":format!("{start:064x}"),"turn":"turn","item":"item","kind":"answer","text":format!("child {start} 日本語"),"complete":true}],"has_more":more})
}

#[test]
fn a_child_display_walks_pages_and_reloads_without_changing_the_captured_task_or_last_accepted_records(
) {
    let (mut client, _, opened) = opened();
    let pending = reply(
        &mut client,
        read(&opened, "task/activity/open"),
        header("a", "c", 1),
    );
    let next = reply(
        &mut client,
        read(&pending, "task/activity/page"),
        page("a", "c", 0, true),
    );
    assert!(client
        .task_activity(&session())
        .expect("pending display")
        .records()
        .is_empty());
    let accepted = reply(
        &mut client,
        read(&next, "task/activity/page"),
        page("a", "c", 1, false),
    );
    assert!(accepted.outgoing.is_empty());
    assert_eq!(records(&client).len(), 2);
    let reload = client
        .advance_task_activity(&session(), false)
        .expect("explicit reload");
    let new_page = reply(
        &mut client,
        read(&reload, "task/activity/advance"),
        header("a", "e", 3),
    );
    assert_eq!(records(&client).len(), 2);
    let _ = reply(
        &mut client,
        read(&new_page, "task/activity/page"),
        page("a", "e", 2, false),
    );
    assert_eq!(
        client
            .task_activity(&session())
            .expect("new display")
            .records()[0]
            .text,
        "child 2 日本語"
    );
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn closing_before_an_open_reply_later_closes_only_its_exact_reader_and_keeps_a_new_reader_alive() {
    let (mut client, _, old) = opened();
    assert!(client.close_task_activity(&session()).outgoing.is_empty());
    let fresh = client
        .open_task_activity(&session(), target())
        .expect("new child reader");
    let _ = reply(
        &mut client,
        read(&fresh, "task/activity/open"),
        header("e", "c", 1),
    );
    let retired = reply(
        &mut client,
        read(&old, "task/activity/open"),
        header("a", "c", 1),
    );
    assert!(retired.replies.is_empty());
    assert!(
        matches!(command(read(&retired,"task/activity/close")).command,Command::TaskActivityClose { activity:Some(ref id), .. } if id.as_str()=="a".repeat(64))
    );
    assert_eq!(
        client
            .task_activity(&session())
            .expect("new reader survives")
            .header()
            .expect("new header")
            .activity
            .as_str(),
        "e".repeat(64)
    );
    let closed = client.close_task_activity(&session());
    assert!(
        matches!(command(read(&closed,"task/activity/close")).command,Command::TaskActivityClose { activity:Some(ref id), .. } if id.as_str()=="e".repeat(64))
    );
    assert!(client.close_task_activity(&session()).outgoing.is_empty());
}

#[test]
fn closing_and_reopening_a_known_reader_emits_the_close_before_the_new_open() {
    let (mut client, _, opened) = opened();
    let _ = reply(
        &mut client,
        read(&opened, "task/activity/open"),
        header("a", "c", 1),
    );
    let replacement = client
        .open_task_activity(&session(), target())
        .expect("explicit replacement");
    let methods: Vec<_> = replacement
        .outgoing
        .iter()
        .map(|outgoing| {
            serde_json::from_slice::<Value>(outgoing.bytes()).expect("request")["method"]
                .as_str()
                .expect("method")
                .to_owned()
        })
        .collect();
    assert_eq!(methods, ["task/activity/close", "task/activity/open"]);
}

#[test]
fn a_failed_page_keeps_the_accepted_display_and_waits_for_explicit_reload_or_a_new_revision() {
    let (mut client, connection, opened) = opened();
    let first = reply(
        &mut client,
        read(&opened, "task/activity/open"),
        header("a", "c", 1),
    );
    let _ = reply(
        &mut client,
        read(&first, "task/activity/page"),
        page("a", "c", 0, false),
    );
    let advance = client
        .advance_task_activity(&session(), true)
        .expect("explicit older items");
    let second = reply(
        &mut client,
        read(&advance, "task/activity/advance"),
        header("a", "e", 2),
    );
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(read(&second, "task/activity/page").id()),
                error: RpcError {
                    code: -32001,
                    message: "private native failure".into(),
                },
            },
        )
        .expect("owned read failure");
    assert!(failed.failures.is_empty());
    assert!(failed.outgoing.is_empty());
    let model = client.task_activity(&session()).expect("failed display");
    assert_eq!(model.records()[0].text, "child 0 日本語");
    assert!(model
        .read_error()
        .expect("failure")
        .contains("private native failure"));
    let retry = client
        .advance_task_activity(&session(), false)
        .expect("explicit retry");
    let pending = reply(
        &mut client,
        read(&retry, "task/activity/advance"),
        header("a", "f", 3),
    );
    let _ = reply(
        &mut client,
        read(&pending, "task/activity/page"),
        page("a", "f", 1, false),
    );
    assert!(client
        .task_activity(&session())
        .expect("recovered")
        .read_error()
        .is_none());
}

#[test]
fn eof_keeps_a_readonly_child_and_neither_reconnect_nor_offline_close_reopens_it() {
    let (mut client, connection, opened) = opened();
    let first = reply(
        &mut client,
        read(&opened, "task/activity/open"),
        header("a", "c", 1),
    );
    let _ = reply(
        &mut client,
        read(&first, "task/activity/page"),
        page("a", "c", 0, false),
    );
    client.disconnect(connection);
    assert!(client
        .task_activity(&session())
        .expect("kept child")
        .is_stale());
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        &hello.outgoing[0],
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(ready.outgoing.is_empty());
    assert_eq!(
        client
            .task_activity(&session())
            .expect("readonly display")
            .records()[0]
            .text,
        "child 0 日本語"
    );
    client.disconnect(hello.outgoing[0].connection());
    assert!(client.close_task_activity(&session()).outgoing.is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    assert!(reply(
        &mut client,
        &hello.outgoing[0],
        json!({"kind":"replay","seq":"10","events":[]})
    )
    .outgoing
    .is_empty());
}

#[test]
fn explicit_unowned_child_opens_keep_their_reply_and_are_never_closed_by_the_panel() {
    let (mut client, connection, opened) = opened();
    let _ = reply(
        &mut client,
        read(&opened, "task/activity/open"),
        header("a", "c", 1),
    );
    let target = target();
    let direct = client
        .read(
            connection,
            Command::TaskActivityOpen {
                session: session(),
                task: target.task,
                expected: target.expected,
                window: target.window,
            },
        )
        .expect("independent native child");
    let unowned = reply(&mut client, &direct, header("f", "c", 1));
    assert!(matches!(
        unowned.replies[0].reply,
        ReadReply::TaskActivity(_)
    ));
    let closed = client.close_task_activity(&session());
    assert!(
        matches!(command(read(&closed,"task/activity/close")).command,Command::TaskActivityClose { activity:Some(ref id), .. } if id.as_str()=="a".repeat(64))
    );
}

fn records(client: &Client) -> &[carlos_proto::task_activity::TaskActivityRecord] {
    client
        .task_activity(&session())
        .expect("accepted activity model")
        .records()
}

#[test]
fn a_closed_native_header_still_releases_its_exact_interest_on_explicit_close() {
    let (mut client, _, opened) = opened();
    let mut closed = header("a", "c", 1);
    closed["closed"] = json!(true);
    let _ = reply(&mut client, read(&opened, "task/activity/open"), closed);
    assert!(client
        .task_activity(&session())
        .expect("native closed")
        .is_stale());
    let effects = client.close_task_activity(&session());
    assert!(matches!(
        command(read(&effects, "task/activity/close")).command,
        Command::TaskActivityClose {
            activity: Some(_),
            ..
        }
    ));
    assert!(client.close_task_activity(&session()).outgoing.is_empty());
}
