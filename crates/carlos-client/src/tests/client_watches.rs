//! Keep body watch replies and resets with the local read lifetime that actually owns them.

use carlos_proto::events::{Event, Notification};
use carlos_proto::output::OutputRevision;
use serde_json::{json, Value};

use super::client::{attached, command, greet, notify, reply};
use super::{entry_id, event, session};
use crate::{BodyToken, Client, ConnectionId};

fn tool(client: &mut Client, connection: ConnectionId, seq: u64, revision: u64) {
    let entry = serde_json::from_value(
        json!({"id":entry_id(10),"kind":"tool","text":"", "done":false,"operation":null,
        "tool":{"kind":"command","label":"tail -f log","label_source":"literal","status":"running",
        "added":null,"removed":null,"has_output":true,"output_revision":revision.to_string()}}),
    )
    .expect("tool facts");
    let _ = notify(
        client,
        connection,
        Notification::SessionEvent(event(seq, Event::EntryUpdated { entry })),
    );
}

fn source(text: &str, from: Option<&str>, next: &str) -> Value {
    json!({"entry":entry_id(10), "offset":"0", "source":{"revision":"3", "bytes":text.len().to_string()},
        "from":from,"next":next,"text":text,"has_more":false,"done":false,"seq":"12"})
}

fn window(client: &mut Client) -> BodyToken {
    client
        .bodies_mut()
        .open_source(session(), entry_id(10), OutputRevision::new(3))
        .expect("source window")
}

#[test]
fn an_old_watch_reset_before_the_replacement_reply_cannot_close_the_new_read() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let old = window(&mut client);
    let first = client.watch_body(connection, &old).expect("first watch");
    let _ = reply(&mut client, &first, source("original", None, "old-end"));
    let current = window(&mut client);
    let second = client
        .watch_body(connection, &current)
        .expect("replacement watch");
    let reset = notify(
        &mut client,
        connection,
        Notification::SourceReset {
            session: session(),
            entry: entry_id(10),
            message: "old watch ended".into(),
        },
    );
    assert!(
        reset.failures.is_empty(),
        "a retired watch cannot report a current failure"
    );
    assert!(client.bodies().source(&current).is_some());
    assert!(
        reset.outgoing.is_empty(),
        "the pending replacement is already queued"
    );
    let accepted = reply(
        &mut client,
        &second,
        source("original extended", None, "new-end"),
    );
    assert_eq!(accepted.bodies, vec![current.clone()]);
    assert_eq!(
        client
            .bodies()
            .source(&current)
            .expect("replacement source")
            .pieces()
            .collect::<String>(),
        "original extended"
    );
}

#[test]
fn closing_a_pending_watch_prevents_its_late_first_reply_from_reviving_the_body() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = window(&mut client);
    let watching = client.watch_body(connection, &token).expect("watch");
    let closed = client.close_body(&token);
    assert_eq!(closed.outgoing.len(), 1);
    assert!(matches!(
        command(&closed.outgoing[0]).command,
        carlos_proto::commands::Command::SourceUnsubscribe { .. }
    ));
    let late = reply(
        &mut client,
        &watching,
        source("late bytes", None, "old-end"),
    );
    assert!(late.bodies.is_empty());
    assert!(client.bodies().source(&token).is_none());
    let update = serde_json::from_value(
        json!({"session":session(),"chunk":source("late bytes", None, "old-end")}),
    )
    .expect("late native output");
    assert!(
        notify(&mut client, connection, Notification::Source(update))
            .bodies
            .is_empty()
    );
}

#[test]
fn output_resets_retire_only_normalized_pieces_and_leave_the_source_watch_alive() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let raw = window(&mut client);
    let read = client.watch_body(connection, &raw).expect("source watch");
    let _ = reply(&mut client, &read, source("raw source", None, "raw-end"));
    let normalized = client
        .bodies_mut()
        .open_output(session(), entry_id(10), OutputRevision::new(3), None)
        .expect("normalized window");
    let read = client
        .watch_body(connection, &normalized)
        .expect("normalized watch");
    let _ = reply(
        &mut client,
        &read,
        json!({"entry":entry_id(10),"revision":"3","from":null,"next":"normalized-end","segments":[],"has_more":false,"done":false,"seq":"12"}),
    );
    let _ = notify(
        &mut client,
        connection,
        Notification::OutputReset {
            session: session(),
            entry: entry_id(10),
            message: "normalized watch ended".into(),
        },
    );
    assert!(client.bodies().output(&normalized).is_none());
    assert_eq!(
        client
            .bodies()
            .source(&raw)
            .expect("source retained")
            .pieces()
            .collect::<String>(),
        "raw source"
    );
}

#[test]
fn a_body_replacement_cancels_the_old_cursor_before_watching_the_new_generation() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = client
        .bodies_mut()
        .open_source_document(session(), entry_id(10), OutputRevision::new(3))
        .expect("document window");
    let read = client.watch_body(connection, &token).expect("old watch");
    let _ = reply(&mut client, &read, source("original", None, "old-end"));
    let replacement = serde_json::from_value(
        json!({"id":entry_id(10),"kind":"tool","text":"", "done":false,"operation":null,
        "tool":{"kind":"command","label":"replacement","label_source":"literal","status":"running",
        "added":null,"removed":null,"has_output":true,"output_revision":"4"}}),
    )
    .expect("new tool facts");
    let changed = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(12, Event::EntryUpdated { entry: replacement })),
    );
    assert_eq!(changed.outgoing.len(), 2);
    assert!(matches!(
        command(&changed.outgoing[0]).command,
        carlos_proto::commands::Command::SourceUnsubscribe { .. }
    ));
    assert!(matches!(
        command(&changed.outgoing[1]).command,
        carlos_proto::commands::Command::SourceSubscribe { cursor: None, .. }
    ));
    assert!(client.bodies().source(&token).is_none());
    let current = changed.bodies.first().expect("replacement token");
    assert_ne!(&token, current);
    assert!(client.bodies().source(current).is_some());
    fill_replacement_document(&mut client, connection, &changed.outgoing[1], current);
}

fn fill_replacement_document(
    client: &mut Client,
    connection: ConnectionId,
    read: &crate::Outbound,
    current: &BodyToken,
) {
    let text = "x".repeat(65536);
    for index in 0..5 {
        let chunk = json!({"entry":entry_id(10), "offset":(index*65536).to_string(),
            "source":{"revision":"4","bytes":(5*65536).to_string()},
            "from":if index == 0 { None } else { Some(format!("at-{index}")) },
            "next":format!("at-{}",index+1),"text":text,
            "has_more":index<4,"done":false,"seq":"12"});
        if index == 0 {
            let _ = reply(client, read, chunk);
        } else {
            let _ = notify(
                client,
                connection,
                Notification::Source(
                    serde_json::from_value(json!({"session":session(),"chunk":chunk}))
                        .expect("live source"),
                ),
            );
        }
    }
    assert_eq!(
        client
            .bodies()
            .source(current)
            .expect("replacement")
            .document()
            .expect("preserved document mode")
            .len(),
        5 * 65536
    );
}

#[test]
fn a_resumed_watch_uses_its_accepted_cursor_after_the_session_replay_and_never_duplicates_source() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let token = window(&mut client);
    let read = client.watch_body(connection, &token).expect("source watch");
    let _ = reply(&mut client, &read, source("raw", None, "raw-end"));
    client.disconnect(connection);
    let (_, welcome) = greet(&mut client, true, "d");
    assert_eq!(
        welcome.outgoing.len(),
        1,
        "session facts restore before the source watch"
    );
    let restored = reply(
        &mut client,
        &welcome.outgoing[0],
        json!({"kind":"replay", "events":[], "seq":"11"}),
    );
    assert_eq!(restored.outgoing.len(), 1);
    assert!(
        matches!(command(&restored.outgoing[0]).command, carlos_proto::commands::Command::SourceSubscribe { cursor:Some(cursor), .. } if cursor.as_str() == "raw-end")
    );
    let applied = reply(
        &mut client,
        &restored.outgoing[0],
        json!({"entry":entry_id(10),"offset":"3", "source":{"revision":"3","bytes":"3"},
        "from":"raw-end","next":"raw-end","text":"","has_more":false,"done":false,"seq":"12"}),
    );
    assert!(applied.bodies.is_empty());
    assert_eq!(
        client
            .bodies()
            .source(&token)
            .expect("resumed source")
            .pieces()
            .collect::<String>(),
        "raw"
    );
}

#[test]
fn old_bulk_output_before_the_replacement_reply_does_not_unsubscribe_the_replacement_watch() {
    let (mut client, connection) = attached();
    tool(&mut client, connection, 11, 3);
    let old = window(&mut client);
    let read = client.watch_body(connection, &old).expect("old watch");
    let _ = reply(&mut client, &read, source("prefix", None, "old-end"));
    let current = window(&mut client);
    let replacement = client
        .watch_body(connection, &current)
        .expect("replacement watch");
    let old_output = serde_json::from_value(json!({"session":session(),"chunk":{"entry":entry_id(10), "offset":"6", "source":{"revision":"3", "bytes":"10"},
        "from":"old-end","next":"old-next","text":"more","has_more":false,"done":false,"seq":"12"}})).expect("old watch output already in flight");
    let ignored = notify(&mut client, connection, Notification::Source(old_output));
    assert!(
        ignored.outgoing.is_empty(),
        "old output cannot queue cancellation of the replacement native watch"
    );
    assert!(client.bodies().source(&current).is_some());
    let _ = reply(
        &mut client,
        &replacement,
        source("prefixmore", None, "new-end"),
    );
    assert_eq!(
        client
            .bodies()
            .source(&current)
            .expect("replacement source")
            .pieces()
            .collect::<String>(),
        "prefixmore"
    );
}

#[path = "client_watch_restore.rs"]
mod restore;

#[path = "client_body_pause.rs"]
mod pause;

#[path = "client_watch_cancellation.rs"]
mod cancellation;
