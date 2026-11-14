//! Restore body watches only after current session facts have been accepted.

use carlos_proto::output::OutputRevision;
use serde_json::json;

use super::{source, tool, window};
use crate::tests::client::{attached, command, greet, reply, subscribe};
use crate::tests::{entry_id, session};
use crate::{BodyToken, Client, ConnectionId};

#[test]
fn a_failed_session_restore_pauses_both_body_formats_until_the_matching_retry_succeeds() {
    for normalized in [false, true] {
        check_failed_restore(normalized);
    }
}

fn check_failed_restore(normalized: bool) {
    let (mut client, connection, token) = accepted_body_watch(normalized);
    client.disconnect(connection);
    check_restore_retry(&mut client, &token);
}

fn accepted_body_watch(normalized: bool) -> (Client, ConnectionId, BodyToken) {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = if normalized {
        client
            .bodies_mut()
            .open_output(session(), entry_id(10), OutputRevision::new(3), None)
            .expect("normalized window")
    } else {
        window(&mut client)
    };
    let read = client.watch_body(connection, &token).expect("body watch");
    let value = if normalized {
        json!({"entry":entry_id(10),"revision":"3","from":null,"next":"raw-end",
            "segments":[],"has_more":false,"done":false,"seq":"12"})
    } else {
        source("raw", None, "raw-end")
    };
    reply(&mut client, &read, value);
    (client, connection, token)
}

fn check_restore_retry(client: &mut Client, token: &BodyToken) {
    let (connection, hello) = greet(client, true, "d");
    assert_eq!(
        hello.outgoing.len(),
        1,
        "hold watches while restore is pending"
    );
    let failed = client
        .receive(
            connection,
            crate::codec::Incoming::Failure {
                id: Some(hello.outgoing[0].id()),
                error: carlos_proto::commands::RpcError {
                    code: -32012,
                    message: "restore rejected".into(),
                },
            },
        )
        .expect("restore failure");
    assert!(
        failed.outgoing.is_empty(),
        "failed restore cannot reopen body watches"
    );
    assert!(!client.session_is_ready(connection, &session()));
    assert!(
        client.bodies().contains(token),
        "keep accepted bytes for offline reading"
    );
    let older = subscribe(client, connection);
    let current = subscribe(client, connection);
    let old = reply(
        client,
        &older,
        json!({"kind":"replay", "events":[], "seq":"11"}),
    );
    assert!(
        old.outgoing.is_empty(),
        "superseded restore cannot reopen watches"
    );
    let restored = reply(
        client,
        &current,
        json!({"kind":"replay", "events":[], "seq":"11"}),
    );
    assert!(client.session_is_ready(connection, &session()));
    assert_eq!(
        restored.outgoing.len(),
        1,
        "restore one requested format exactly once"
    );
    assert_resumed_cursor(&restored.outgoing[0]);
}

fn assert_resumed_cursor(request: &crate::Outbound) {
    match command(request).command {
        carlos_proto::commands::Command::SourceSubscribe {
            cursor: Some(cursor),
            ..
        } => assert_eq!(cursor.as_str(), "raw-end"),
        carlos_proto::commands::Command::OutputSubscribe {
            cursor: Some(cursor),
            ..
        } => assert_eq!(cursor.as_str(), "raw-end"),
        _ => panic!("expected source or output watch"),
    }
}
