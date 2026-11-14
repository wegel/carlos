//! Recover read interests and private outcomes across socket generations without repeating writes.

use carlos_proto::commands::Command;
use carlos_proto::events::{Event, Notification};
use carlos_proto::submissions::OperationState;
use serde_json::json;

use super::client::{attached, command, greet, notify, reply};
use super::{entry_id, event, session};
use crate::codec::Incoming;
use crate::Welcome;

fn resumed_pending() -> (
    crate::Client,
    crate::ConnectionId,
    crate::ConnectionId,
    carlos_proto::submissions::ClientOperation,
    crate::Effects,
) {
    let (mut client, first) = attached();
    let submitted = client
        .submit(first, |operation| Command::Send {
            session: session(),
            operation,
            text: "possibly written text".into(),
            policy: Default::default(),
        })
        .expect("explicit send");
    let Command::Send { operation, .. } = command(&submitted).command else {
        panic!("send");
    };
    client.disconnect(first);
    let (second, welcome) = greet(&mut client, true, "d");
    (client, first, second, operation, welcome)
}

#[test]
fn resumed_connections_replay_facts_and_query_missing_private_outcomes_without_a_new_send() {
    let (mut client, _first, second, operation, mut welcome) = resumed_pending();
    assert_eq!(welcome.welcome, Some(Welcome::Resumed));
    assert_eq!(welcome.outgoing.len(), 2);
    let subscription = welcome.outgoing.remove(0);
    assert!(
        matches!(command(&subscription).command, Command::Subscribe { since: Some(cursor), .. } if cursor.get() == 10)
    );
    let recovery = welcome.outgoing.remove(0);
    assert!(
        matches!(command(&recovery).command, Command::Operation { operation: ref read } if read == &operation)
    );
    reply(
        &mut client,
        &subscription,
        json!({"kind":"replay", "events":[], "seq":"10"}),
    );
    let absent = client
        .receive(
            second,
            Incoming::Failure {
                id: Some(recovery.id()),
                error: carlos_proto::commands::RpcError {
                    code: -32006,
                    message: "operation is unknown".into(),
                },
            },
        )
        .expect("unknown private outcome");
    assert!(absent.outgoing.is_empty());
    assert!(matches!(
        client
            .operations()
            .get(&operation)
            .expect("recovered origin")
            .state,
        OperationState::Uncertain { .. }
    ));
    assert_eq!(
        client
            .operations()
            .get(&operation)
            .expect("retained draft")
            .text(),
        Some("possibly written text")
    );
}

#[test]
fn daemon_replacement_makes_pending_text_uncertain_and_uses_full_snapshots_for_old_sessions() {
    let (mut client, connection) = attached();
    let sent = client
        .submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "do not replay".into(),
            policy: Default::default(),
        })
        .expect("send");
    let Command::Send { operation, .. } = command(&sent).command else {
        panic!("send");
    };
    client.disconnect(connection);
    let (_, welcome) = greet(&mut client, false, "1");
    assert_eq!(
        welcome.welcome,
        Some(Welcome::Replaced {
            instance_changed: true
        })
    );
    assert_eq!(welcome.outgoing.len(), 1);
    assert!(matches!(
        command(&welcome.outgoing[0]).command,
        Command::Subscribe { since: None, .. }
    ));
    assert!(matches!(
        client
            .operations()
            .get(&operation)
            .expect("retained origin")
            .state,
        OperationState::Uncertain { .. }
    ));
    assert_eq!(
        client
            .operations()
            .get(&operation)
            .expect("retained draft")
            .text(),
        Some("do not replay")
    );
}

#[test]
fn frames_from_an_old_socket_cannot_change_facts_and_resumed_counters_never_reuse_an_issued_id() {
    let (mut client, first, second, _operation, _welcome) = resumed_pending();
    let old = notify(
        &mut client,
        first,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "old socket".into(),
            },
        )),
    );
    assert!(old.sessions.is_empty());
    assert_eq!(
        client
            .session(&session())
            .expect("current socket facts")
            .sequence()
            .get(),
        10
    );
    let next = client
        .submit(second, |operation| Command::Rename {
            session: session(),
            operation,
            name: "next explicit action".into(),
        })
        .expect("next user command");
    assert!(
        matches!(command(&next).command, Command::Rename { operation, .. } if operation.id.get() == 2)
    );
}

#[test]
fn definite_and_possible_write_failures_preserve_text_but_only_possible_writes_need_an_outcome_query(
) {
    for possibly_written in [false, true] {
        let (mut client, connection) = attached();
        let request = client
            .submit(connection, |operation| Command::Send {
                session: session(),
                operation,
                text: "recover exact text".into(),
                policy: Default::default(),
            })
            .expect("explicit work");
        let Command::Send { operation, .. } = command(&request).command else {
            panic!("send");
        };
        client.write_failed(
            connection,
            request.id(),
            "platform writer stopped",
            possibly_written,
        );
        assert_eq!(
            client
                .operations()
                .get(&operation)
                .expect("retained draft")
                .text(),
            Some("recover exact text")
        );
        let (_, welcome) = greet(&mut client, true, "d");
        let queries = welcome
            .outgoing
            .iter()
            .filter(|outgoing| matches!(command(outgoing).command, Command::Operation { .. }))
            .count();
        assert_eq!(queries, usize::from(possibly_written));
        assert!(welcome.outgoing.iter().all(|outgoing| matches!(
            command(outgoing).command,
            Command::Subscribe { .. } | Command::Operation { .. }
        )));
    }
}

#[test]
fn a_late_platform_write_failure_cannot_undo_authoritative_private_acceptance() {
    let (mut client, connection) = attached();
    let request = client
        .submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "already accepted".into(),
            policy: Default::default(),
        })
        .expect("explicit work");
    let Command::Send { operation, .. } = command(&request).command else {
        panic!("send");
    };
    let result = serde_json::from_value(
        json!({"operation":operation,"session":session(),"state":{"kind":"accepted","entry":null}}),
    )
    .expect("native acceptance");
    notify(&mut client, connection, Notification::Operation(result));
    let failed = client.write_failed(connection, request.id(), "late writer callback", true);
    assert!(failed.operations.is_empty());
    assert!(matches!(
        client
            .operations()
            .get(&operation)
            .expect("authoritative result")
            .state,
        OperationState::Accepted { .. }
    ));
    let (_, welcome) = greet(&mut client, true, "d");
    assert!(welcome
        .outgoing
        .iter()
        .all(|outgoing| matches!(command(outgoing).command, Command::Subscribe { .. })));
}
