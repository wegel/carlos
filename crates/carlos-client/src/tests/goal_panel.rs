//! Settle one goal copy through owned reads and preserve watch closure before a native reply.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use serde_json::json;

use super::client::{attached, command, greet, notify, reply};
use super::queue_panel::single_read;
use super::{entry_id, event, session};
use crate::codec::Incoming;
use crate::ReadReply;

#[test]
fn an_owned_goal_reply_waits_for_its_numbered_facts_without_exposing_another_goal_copy() {
    let (mut client, connection) = attached();
    let opened = client.open_goal_panel(&session()).expect("goal reader");
    assert!(matches!(
        command(single_read(&opened)).command,
        Command::GoalSubscribe { .. }
    ));
    let future = reply(&mut client, single_read(&opened), snapshot(11, 'a'));
    assert!(future.replies.is_empty());
    assert!(future.outgoing.is_empty());
    assert!(client.session(&session()).expect("facts").goal().is_none());
    assert!(client.goal_panel(&session()).expect("reader").is_reading());
    let arrived = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "".into(),
            },
        )),
    );
    assert!(arrived.outgoing.is_empty());
    assert!(!client
        .goal_panel(&session())
        .expect("settled reader")
        .is_reading());
    assert_eq!(
        client
            .session(&session())
            .expect("facts")
            .goal()
            .expect("one goal")
            .revision,
        "a".repeat(64).parse().expect("revision")
    );
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn closing_before_the_initial_reply_sends_one_unsubscribe_and_ignores_late_data_and_errors() {
    let (mut client, _) = attached();
    let old = client.open_goal_panel(&session()).expect("initial read");
    let closed = client.close_goal_panel(&session());
    assert!(matches!(
        command(single_read(&closed)).command,
        Command::GoalUnsubscribe { .. }
    ));
    assert!(client.close_goal_panel(&session()).outgoing.is_empty());
    let late = reply(&mut client, single_read(&old), snapshot(10, 'a'));
    assert!(late.replies.is_empty());
    assert!(client.session(&session()).expect("facts").goal().is_none());
    let fresh = client.open_goal_panel(&session()).expect("fresh reader");
    let ignored = client
        .receive(
            single_read(&old).connection(),
            Incoming::Failure {
                id: Some(single_read(&old).id()),
                error: RpcError {
                    code: -32001,
                    message: "obsolete goal error".into(),
                },
            },
        )
        .expect("retired failure");
    assert!(ignored.failures.is_empty());
    let _ = reply(&mut client, single_read(&fresh), snapshot(10, 'b'));
    assert!(client
        .goal_panel(&session())
        .expect("new reader")
        .read_error()
        .is_none());
}

#[test]
fn repeated_explicit_refresh_during_session_restore_keeps_one_unissued_read_intent() {
    let (mut client, connection) = attached();
    let opened = client.open_goal_panel(&session()).expect("open");
    let _ = reply(&mut client, single_read(&opened), snapshot(10, 'a'));
    let restoring = client
        .read(
            connection,
            Command::Subscribe {
                session: session(),
                since: Some(Sequence::new(10)),
            },
        )
        .expect("explicit session restore");
    for _ in 0..2 {
        assert!(client
            .refresh_goal_panel(&session())
            .expect("refresh after restore")
            .outgoing
            .is_empty());
    }
    let ready = reply(
        &mut client,
        &restoring,
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(matches!(
        command(single_read(&ready)).command,
        Command::GoalRead { refresh: true, .. }
    ));
    let _ = reply(&mut client, single_read(&ready), snapshot(10, 'a'));
    assert!(!client.goal_panel(&session()).expect("reader").is_reading());
}

#[test]
fn a_failed_initial_watch_closes_the_native_reader_and_only_explicit_refresh_reopens_it() {
    let (mut client, connection) = attached();
    let opened = client.open_goal_panel(&session()).expect("open");
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&opened).id()),
                error: RpcError {
                    code: -32001,
                    message: "native goal read failed".into(),
                },
            },
        )
        .expect("owned failure");
    assert!(failed.failures.is_empty());
    assert!(matches!(
        command(single_read(&failed)).command,
        Command::GoalUnsubscribe { .. }
    ));
    assert_eq!(
        client.goal_panel(&session()).expect("reader").read_error(),
        Some("native goal read failed")
    );
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
    let explicit = client
        .refresh_goal_panel(&session())
        .expect("new explicit interest");
    assert!(matches!(
        command(single_read(&explicit)).command,
        Command::GoalSubscribe { .. }
    ));
    let _ = reply(&mut client, single_read(&explicit), snapshot(11, 'a'));
    assert!(client
        .goal_panel(&session())
        .expect("reader")
        .read_error()
        .is_none());
}

#[test]
fn independent_goal_reads_remain_unowned_and_offline_close_does_not_restore_a_watch() {
    let (mut client, connection) = attached();
    let opened = client.open_goal_panel(&session()).expect("open");
    let _ = reply(&mut client, single_read(&opened), snapshot(10, 'a'));
    let direct = client
        .read(
            connection,
            Command::GoalRead {
                session: session(),
                refresh: true,
            },
        )
        .expect("independent read");
    let explicit = reply(&mut client, &direct, snapshot(10, 'b'));
    assert!(matches!(explicit.replies[0].reply, ReadReply::Goal(_)));
    assert_eq!(
        client
            .session(&session())
            .expect("facts")
            .goal()
            .expect("goal")
            .revision,
        "a".repeat(64).parse().expect("unchanged revision")
    );
    client.disconnect(connection);
    assert!(client.close_goal_panel(&session()).outgoing.is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(ready.outgoing.is_empty());
    assert!(client.goal_panel(&session()).is_none());
}

fn snapshot(seq: u64, revision: char) -> serde_json::Value {
    json!({"session":session(),"seq":seq.to_string(),"goal":{
        "revision":revision.to_string().repeat(64),"knowledge":"known","goal":null,
        "reading":false,"pending":false,"direct_input":true,"read_error":null,"notice":""
    }})
}
