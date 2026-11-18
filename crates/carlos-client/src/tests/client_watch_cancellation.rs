//! Ignore replies from cancelled watches while preserving errors from the current reader.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::output::OutputRevision;
use serde_json::json;

use super::{tool, window};
use crate::codec::Incoming;
use crate::tests::client::{attached, reply};
use crate::tests::{entry_id, session};
use crate::{BodyToken, Client, ConnectionId, Effects, Outbound};

#[test]
fn a_direct_read_failure_cannot_survive_its_body_version() {
    for normalized in [true, false] {
        let (mut client, connection) = attached();
        tool(&mut client, connection, 11, 3);
        let token = open(&mut client, normalized);
        let old = client.read_body(connection, &token).expect("old body read");
        tool(&mut client, connection, 12, 4);
        let ignored = client
            .receive(
                connection,
                Incoming::Failure {
                    id: Some(old.id()),
                    error: RpcError {
                        code: -32012,
                        message: "output body replaced; restart without a cursor".into(),
                    },
                },
            )
            .expect("late replaced body error");
        assert!(ignored.failures.is_empty());
        assert!(!client.bodies().contains(&token));
        let current = if normalized {
            client
                .bodies_mut()
                .open_output(session(), entry_id(10), OutputRevision::new(4), None)
        } else {
            client
                .bodies_mut()
                .open_source(session(), entry_id(10), OutputRevision::new(4))
        }
        .expect("replacement body starts fresh");
        let read = client
            .read_body(connection, &current)
            .expect("new body read");
        let failed = client
            .receive(
                connection,
                Incoming::Failure {
                    id: Some(read.id()),
                    error: RpcError {
                        code: -32012,
                        message: "current body read failed".into(),
                    },
                },
            )
            .expect("current failure");
        assert_eq!(failed.failures.len(), 1);
    }
}

fn open(client: &mut Client, normalized: bool) -> BodyToken {
    if normalized {
        client
            .bodies_mut()
            .open_output(session(), entry_id(10), OutputRevision::new(3), None)
            .expect("normalized window")
    } else {
        window(client)
    }
}

fn reject(client: &mut Client, connection: ConnectionId, watch: &Outbound) -> Effects {
    client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(watch.id()),
                error: RpcError {
                    code: -32012,
                    message: "output watch was superseded".into(),
                },
            },
        )
        .expect("a request failure cannot break the transport")
}

#[test]
fn cancelled_and_replaced_watch_failures_cannot_touch_the_current_body() {
    for normalized in [false, true] {
        let (mut client, connection) = attached();
        tool(&mut client, connection, 11, 3);
        let old = open(&mut client, normalized);
        let first = client.watch_body(connection, &old).expect("first watch");
        let _ = client.close_body(&old);
        let current = open(&mut client, normalized);
        let second = client
            .watch_body(connection, &current)
            .expect("replacement");
        let ignored = reject(&mut client, connection, &first);
        assert!(ignored.failures.is_empty());
        assert!(ignored.bodies.is_empty());
        assert!(ignored.outgoing.is_empty());
        assert!(client.bodies().contains(&current));
        let failed = reject(&mut client, connection, &second);
        assert_eq!(failed.failures.len(), 1, "current failures remain visible");
        assert!(!client.bodies().contains(&current));
        assert!(client.is_ready(connection));
    }
}

#[test]
fn rewatching_the_same_token_retires_only_its_earlier_request() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = window(&mut client);
    let first = client.watch_body(connection, &token).expect("first watch");
    let second = client.watch_body(connection, &token).expect("newer watch");
    assert!(reject(&mut client, connection, &first).failures.is_empty());
    assert!(client.bodies().contains(&token));
    assert_eq!(reject(&mut client, connection, &second).failures.len(), 1);
}

#[test]
fn pausing_a_pending_watch_keeps_its_body_and_ignores_the_late_error() {
    for normalized in [false, true] {
        let (mut client, connection) = attached();
        tool(&mut client, connection, 11, 3);
        let token = open(&mut client, normalized);
        let watch = client
            .watch_body(connection, &token)
            .expect("pending watch");
        let command = if normalized {
            Command::OutputUnsubscribe {
                session: session(),
                entry: entry_id(10),
            }
        } else {
            Command::SourceUnsubscribe {
                session: session(),
                entry: entry_id(10),
            }
        };
        let _ = client.read(connection, command).expect("explicit pause");
        let ignored = reject(&mut client, connection, &watch);
        assert!(ignored.failures.is_empty());
        assert!(ignored.bodies.is_empty());
        assert!(ignored.outgoing.is_empty());
        assert!(client.bodies().contains(&token));
        assert!(client.is_ready(connection));
    }
}

#[test]
fn a_late_watch_success_cannot_apply_its_old_cursor_after_a_paused_body_has_advanced() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = window(&mut client);
    let watch = client
        .watch_body(connection, &token)
        .expect("pending watch");
    let _ = client
        .read(
            connection,
            Command::SourceUnsubscribe {
                session: session(),
                entry: entry_id(10),
            },
        )
        .expect("pause the watch before pulling the backlog");
    let read = client
        .read_body(connection, &token)
        .expect("direct backlog read");
    let chunk = |text: &str, next: &str| {
        json!({"entry":entry_id(10),"offset":"0",
        "source":{"revision":"3","bytes":"12"},"text":text,"from":null,
        "next":next,"has_more":text.len()<12,"done":false,"seq":"12"})
    };
    let _ = reply(&mut client, &read, chunk("full backlog", "tail"));
    let late = reply(&mut client, &watch, chunk("full", "old-middle"));
    assert!(late.bodies.is_empty());
    assert!(late.failures.is_empty());
    assert_eq!(
        client
            .bodies()
            .source(&token)
            .expect("retained backlog")
            .pieces()
            .collect::<String>(),
        "full backlog"
    );
    assert!(client.is_ready(connection));
}
