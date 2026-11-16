//! Keep cache failures local to reads while preserving transport rights and submitted text.

use carlos_proto::commands::Command;
use carlos_proto::events::{Event, Notification};
use carlos_proto::output::OutputRevision;
use serde_json::json;

use super::client::{attached, command, notify, reply};
use super::{entry, entry_id, event, session};
use crate::{Client, ConnectionId};

fn pressure(client: &mut Client, connection: ConnectionId, watch: bool) {
    let token = client
        .bodies_mut()
        .open_source_document(session(), entry_id(10), OutputRevision::new(3))
        .expect("contiguous window");
    let read = if watch {
        client.watch_body(connection, &token)
    } else {
        client.read_body(connection, &token)
    }
    .expect("body request");
    let effects = reply(
        client,
        &read,
        json!({"entry":entry_id(10),"offset":"0",
        "source":{"revision":"3","bytes":"8388609"},"from":null,"next":"next",
        "text":"x","has_more":true,"done":false,"seq":"10"}),
    );
    assert_eq!(effects.failures.len(), 1);
    assert_eq!(effects.failures[0].id, Some(read.id()));
    assert_eq!(effects.outgoing.len(), usize::from(watch));
    if watch {
        assert!(matches!(
            command(&effects.outgoing[0]).command,
            Command::SourceUnsubscribe { .. }
        ));
        let _ = reply(client, &effects.outgoing[0], json!(null));
    } else {
        assert!(client
            .bodies()
            .source(&token)
            .expect("retryable window")
            .descriptor()
            .is_none());
    }
    let later = notify(
        client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::EntryAdded {
                entry: entry(11, "another row"),
            },
        )),
    );
    assert!(
        later.outgoing.is_empty(),
        "cache pressure cannot loop subscriptions or send backend writes"
    );
    assert!(!client
        .session(&session())
        .expect("retained facts")
        .is_stale());
    assert_eq!(client.bodies().source(&token).is_some(), !watch);
}

#[test]
fn a_failed_document_read_preserves_private_text_and_keeps_the_connection_usable() {
    for watch in [false, true] {
        let (mut client, connection) = attached();
        let send = client
            .submit(connection, |operation| Command::Send {
                session: session(),
                operation,
                text: "exact submitted draft 🌍".into(),
                policy: Default::default(),
            })
            .expect("submit private text");
        let Command::Send { operation, .. } = command(&send).command else {
            panic!("send");
        };
        pressure(&mut client, connection, watch);
        let private = client
            .operations()
            .get(&operation)
            .expect("preserved operation");
        assert_eq!(private.text(), Some("exact submitted draft 🌍"));
        assert!(private.needs_echo());
        client
            .read(connection, Command::Watch {})
            .expect("same connection can read");
    }
}

fn live_chunk(source: bool, appended: bool) -> serde_json::Value {
    let seq = if appended { "12" } else { "11" };
    let from = appended.then_some("first");
    let next = if appended { "second" } else { "first" };
    let text = if appended { "x" } else { "base" };
    if source {
        json!({"entry":entry_id(10),"offset":if appended { "4" } else { "0" },
            "source":{"revision":"3","bytes":if appended { "5" } else { "4" }},
            "from":from,"next":next,"text":text,"has_more":false,"done":false,"seq":seq})
    } else {
        json!({"entry":entry_id(10),"revision":"3","from":from,"next":next,
            "segments":[{"kind":"plain","text":text}],"has_more":false,"done":false,"seq":seq})
    }
}

#[test]
fn live_cache_pressure_cancels_only_the_failed_format_without_restarting_it() {
    for source in [false, true] {
        let (mut client, connection) = attached();
        client
            .submit(connection, |operation| Command::Send {
                session: session(),
                operation,
                text: "private draft".into(),
                policy: Default::default(),
            })
            .expect("private prompt");
        let token = if source {
            client.bodies_mut().open_source_document(
                session(),
                entry_id(10),
                OutputRevision::new(3),
            )
        } else {
            client
                .bodies_mut()
                .open_output(session(), entry_id(10), OutputRevision::new(3), None)
        }
        .expect("watched window");
        let watch = client.watch_body(connection, &token).expect("watch");
        let _ = reply(&mut client, &watch, live_chunk(source, false));
        let other = client
            .bodies_mut()
            .open_source_document(session(), entry_id(20), OutputRevision::new(3))
            .expect("another document");
        super::body_documents::fill(client.bodies_mut(), &other, 8 * 1024 * 1024 - 4);
        fail_live_watch(&mut client, connection, source);
        assert!(client.bodies().source(&token).is_none());
        assert!(client.bodies().output(&token).is_none());
        assert_eq!(
            client
                .bodies()
                .source(&other)
                .expect("other reader retained")
                .document()
                .expect("complete other reader")
                .len(),
            8 * 1024 * 1024 - 4
        );
        assert_eq!(
            client
                .operations()
                .iter()
                .next()
                .expect("submitted prompt")
                .text(),
            Some("private draft")
        );
    }
}

fn fail_live_watch(client: &mut Client, connection: ConnectionId, source: bool) {
    let update = json!({"session":session(),"chunk":live_chunk(source, true)});
    let notification = if source {
        Notification::Source(serde_json::from_value(update).expect("source update"))
    } else {
        Notification::Output(serde_json::from_value(update).expect("output update"))
    };
    let failed = notify(client, connection, notification.clone());
    assert_eq!(failed.failures.len(), 1);
    assert_eq!(failed.outgoing.len(), 1);
    match command(&failed.outgoing[0]).command {
        Command::SourceUnsubscribe { entry, .. } => {
            assert!(source);
            assert_eq!(entry, entry_id(10));
        }
        Command::OutputUnsubscribe { entry, .. } => {
            assert!(!source);
            assert_eq!(entry, entry_id(10));
        }
        _ => panic!("only failed watch should stop"),
    }
    let _ = reply(client, &failed.outgoing[0], json!(null));
    let late = notify(client, connection, notification);
    assert!(late.outgoing.is_empty());
    assert!(late.bodies.is_empty());
    assert!(late.failures.is_empty());
}
