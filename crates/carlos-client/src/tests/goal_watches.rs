//! Keep goal read lifetimes independent of transport reconnects and local panel closure.

use carlos_proto::commands::Command;
use carlos_proto::session::Snapshot;
use serde_json::json;

use super::client::{attached, command, greet, reply};
use super::{session, snapshot};

#[test]
fn reconnect_restores_goal_reads_after_session_facts_and_offline_close_retires_them() {
    let (mut client, connection) = attached();
    client
        .read(connection, Command::GoalSubscribe { session: session() })
        .expect("goal interest");
    client.disconnect(connection);
    let (_, hello) = greet(&mut client, true, "d");
    assert!(hello
        .outgoing
        .iter()
        .all(|request| !matches!(command(request).command, Command::GoalSubscribe { .. })));
    let request = hello
        .outgoing
        .iter()
        .find(|request| matches!(command(request).command, Command::Subscribe { .. }))
        .expect("restored session");
    let effects = reply(
        &mut client,
        request,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    assert_eq!(
        effects
            .outgoing
            .iter()
            .filter(|request| matches!(command(request).command, Command::GoalSubscribe { .. }))
            .count(),
        1
    );
    client.disconnect(request.connection());
    assert!(client.close_goal(&session()).outgoing.is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    let request = hello
        .outgoing
        .iter()
        .find(|request| matches!(command(request).command, Command::Subscribe { .. }))
        .expect("facts retained");
    let effects = reply(
        &mut client,
        request,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    assert!(!effects
        .outgoing
        .iter()
        .any(|request| matches!(command(request).command, Command::GoalSubscribe { .. })));
}

#[test]
fn close_before_goal_reply_sends_one_unsubscribe_and_never_reopens_the_panel() {
    let (mut client, connection) = attached();
    let pending = client
        .read(connection, Command::GoalSubscribe { session: session() })
        .expect("pending goal read");
    let closed = client.close_goal(&session());
    assert_eq!(closed.outgoing.len(), 1);
    assert!(matches!(
        command(&closed.outgoing[0]).command,
        Command::GoalUnsubscribe { .. }
    ));
    assert!(client.close_goal(&session()).outgoing.is_empty());
    let page: Snapshot = snapshot();
    let facts = json!({"revision":"a".repeat(64), "knowledge":"known", "goal":null,
        "reading":false, "pending":false, "direct_input":true,"read_error":null,"notice":""});
    let effects = reply(
        &mut client,
        &pending,
        json!({"session":session(),"seq":page.seq,"goal":facts}),
    );
    assert!(effects.outgoing.is_empty());
    client.disconnect(connection);
    let (_, hello) = greet(&mut client, true, "d");
    let request = hello
        .outgoing
        .iter()
        .find(|request| matches!(command(request).command, Command::Subscribe { .. }))
        .expect("session restore");
    let effects = reply(
        &mut client,
        request,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    assert!(!effects
        .outgoing
        .iter()
        .any(|request| matches!(command(request).command, Command::GoalSubscribe { .. })));
}

#[test]
fn a_failed_admitted_subscribe_cancels_its_server_reader_without_cancelling_a_newer_one() {
    let (mut client, connection) = attached();
    let first = client
        .watch_goal(&session())
        .expect("first reader")
        .outgoing
        .remove(0);
    let failure = client
        .receive(
            connection,
            crate::codec::Incoming::Failure {
                id: Some(first.id()),
                error: carlos_proto::commands::RpcError {
                    code: -32012,
                    message: "owner read failed".into(),
                },
            },
        )
        .expect("failed read");
    assert_eq!(failure.outgoing.len(), 1);
    assert!(matches!(
        command(&failure.outgoing[0]).command,
        Command::GoalUnsubscribe { .. }
    ));
    assert!(client.pending_goal_read(&session()).is_none());
    let old = client
        .watch_goal(&session())
        .expect("old pending reader")
        .outgoing
        .remove(0);
    let _ = client.close_goal(&session());
    let new = client
        .watch_goal(&session())
        .expect("new reader")
        .outgoing
        .remove(0);
    let failed = client
        .receive(
            connection,
            crate::codec::Incoming::Failure {
                id: Some(old.id()),
                error: carlos_proto::commands::RpcError {
                    code: -32012,
                    message: "old read failed".into(),
                },
            },
        )
        .expect("late failure");
    assert!(failed.outgoing.is_empty());
    assert_eq!(client.pending_goal_read(&session()), Some(new.id()));
}
