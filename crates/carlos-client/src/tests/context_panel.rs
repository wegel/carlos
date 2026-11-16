//! Keep host default reads scoped to a wanted reader, separate from active model usage and writes.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use serde_json::json;

use super::client::{attached, command, greet, notify, reply};
use super::queue_panel::single_read;
use super::{entry_id, event, session};
use crate::codec::Incoming;
use crate::ReadReply;

#[test]
fn a_host_default_transfers_to_an_editor_without_changing_live_context_or_creating_work() {
    let (mut client, _) = attached();
    let opened = client
        .open_context_panel(&session())
        .expect("host default interest");
    assert!(matches!(
        command(single_read(&opened)).command,
        Command::ContextRead { .. }
    ));
    let accepted = reply(
        &mut client,
        single_read(&opened),
        json!({"session":session(),"saved_tokens":200000,"policy":"owned"}),
    );
    assert!(accepted.replies.is_empty());
    assert!(accepted.outgoing.is_empty());
    assert!(!client
        .context_panel(&session())
        .expect("panel")
        .is_reading());
    let defaults = client
        .take_context_defaults(&session())
        .expect("one result")
        .expect("checked default");
    assert_eq!(defaults.saved_tokens, Some(200000));
    assert!(client.take_context_defaults(&session()).is_none());
    assert!(client
        .session(&session())
        .expect("live facts")
        .info()
        .context
        .is_none());
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn offline_interest_waits_for_current_subscription_and_offline_close_prevents_restore() {
    let (mut client, connection) = attached();
    client.disconnect(connection);
    assert!(client
        .open_context_panel(&session())
        .expect("offline interest")
        .outgoing
        .is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    assert!(matches!(
        command(single_read(&hello)).command,
        Command::Subscribe { .. }
    ));
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(matches!(
        command(single_read(&ready)).command,
        Command::ContextRead { .. }
    ));
    client.disconnect(single_read(&ready).connection());
    assert!(client.close_context_panel(&session()).outgoing.is_empty());
    let (_, hello) = greet(&mut client, true, "d");
    let ready = reply(
        &mut client,
        single_read(&hello),
        json!({"kind":"replay","seq":"10","events":[]}),
    );
    assert!(ready.outgoing.is_empty());
    assert!(client.context_panel(&session()).is_none());
}

#[test]
fn retired_errors_and_explicit_reads_cannot_consume_a_reopened_readers_result() {
    let (mut client, connection) = attached();
    let old = client.open_context_panel(&session()).expect("old reader");
    client.close_context_panel(&session());
    let current = client.open_context_panel(&session()).expect("new reader");
    let late = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&old).id()),
                error: RpcError {
                    code: -32001,
                    message: "obsolete failure".into(),
                },
            },
        )
        .expect("retired id");
    assert!(late.failures.is_empty());
    let direct = client
        .read(connection, Command::ContextRead { session: session() })
        .expect("explicit independent read");
    let unowned = reply(
        &mut client,
        &direct,
        json!({"session":session(),"saved_tokens":null,"policy":"unsupported"}),
    );
    assert!(matches!(
        unowned.replies[0].reply,
        ReadReply::ContextDefaults(_)
    ));
    assert!(client
        .context_panel(&session())
        .expect("new reader")
        .is_reading());
    reply(
        &mut client,
        single_read(&current),
        json!({"session":session(),"saved_tokens":null,"policy":"owned"}),
    );
    assert!(client
        .take_context_defaults(&session())
        .expect("owned result")
        .is_ok());
}

#[test]
fn owned_errors_wait_for_explicit_refresh_without_allocating_native_work() {
    let (mut client, connection) = attached();
    let current = client.open_context_panel(&session()).expect("owned reader");
    let failed = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(single_read(&current).id()),
                error: RpcError {
                    code: -32001,
                    message: "host default unavailable".into(),
                },
            },
        )
        .expect("owned failure");
    assert!(failed.failures.is_empty());
    assert_eq!(
        client
            .take_context_defaults(&session())
            .expect("explicit error")
            .expect_err("failed read"),
        "host default unavailable"
    );
    let unrelated = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::TextAppended {
                entry: entry_id(10),
                text: "unrelated text".into(),
            },
        )),
    );
    assert!(unrelated.outgoing.is_empty());
    let refreshed = client
        .refresh_context_panel(&session())
        .expect("explicit retry");
    reply(
        &mut client,
        single_read(&refreshed),
        json!({"session":session(),"saved_tokens":null,"policy":"attached_codex"}),
    );
    assert!(client
        .take_context_defaults(&session())
        .expect("checked result")
        .expect("attached policy")
        .saved_tokens
        .is_none());
    assert!(client.operations().iter().next().is_none());
}

#[test]
fn request_capacity_failure_does_not_start_a_refetch_loop_when_an_unrelated_slot_frees() {
    let (mut client, connection) = attached();
    let requests: Vec<_> = (0..128)
        .map(|_| {
            client
                .read(connection, Command::Watch {})
                .expect("bounded pending slot")
        })
        .collect();
    let opened = client
        .open_context_panel(&session())
        .expect("wanted reader reports local capacity");
    assert!(opened.outgoing.is_empty());
    assert!(client
        .context_panel(&session())
        .expect("explicit reader error")
        .read_error()
        .is_some());
    let freed = reply(&mut client, &requests[0], json!({"sessions":[]}));
    assert!(freed.outgoing.is_empty());
    let retry = client
        .refresh_context_panel(&session())
        .expect("explicit retry after capacity frees");
    assert!(matches!(
        command(single_read(&retry)).command,
        Command::ContextRead { .. }
    ));
}
