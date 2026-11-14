//! Reject crossed read targets and defer future history until its numbered facts arrive.

use carlos_proto::bytes::ByteOffset;
use carlos_proto::commands::Command;
use carlos_proto::events::{Event, Notification};
use serde_json::json;

use super::client::{attached, notify, reply};
use super::{entry, entry_id, event, session};
use crate::codec::Incoming;
use crate::{ClientError, ReadReply};

#[test]
fn a_future_history_reply_waits_for_its_fixed_cursor_while_new_live_text_keeps_advancing() {
    let (mut client, connection) = attached();
    let read = client
        .read(
            connection,
            Command::History {
                session: session(),
                before: entry_id(10),
                limit: 20,
            },
        )
        .expect("older page");
    let held = reply(
        &mut client,
        &read,
        json!({"entries":[entry(9,"older")], "has_older":false, "seq":"12"}),
    );
    assert!(held.sessions.is_empty());
    assert_eq!(
        client.session(&session()).expect("facts").entries().count(),
        1
    );
    append(&mut client, connection, 11, " first");
    let reached = append(&mut client, connection, 12, " second");
    assert_eq!(
        reached.sessions.len(),
        2,
        "the live delta and ready older page both change rows"
    );
    assert!(
        reached.outgoing.is_empty(),
        "a continuously advancing backend does not force another history request"
    );
    let replica = client.session(&session()).expect("current facts");
    assert_eq!(
        replica.entry(&entry_id(10)).expect("live row").text,
        "hello first second"
    );
    assert_eq!(
        replica.entry(&entry_id(9)).expect("older row").text,
        "older"
    );
    assert_eq!(replica.sequence().get(), 12);
}

#[test]
fn a_changed_unseen_history_row_rejects_only_that_page_and_keeps_the_connection_usable() {
    let (mut client, connection) = attached();
    let read = client
        .read(
            connection,
            Command::History {
                session: session(),
                before: entry_id(10),
                limit: 20,
            },
        )
        .expect("older page");
    notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            11,
            Event::EntryUpdated {
                entry: entry(9, "newer unseen text"),
            },
        )),
    );
    let rejected = reply(
        &mut client,
        &read,
        json!({"entries":[entry(9,"old text")], "has_older":false, "seq":"10"}),
    );
    assert_eq!(rejected.failures.len(), 1);
    assert_eq!(rejected.failures[0].id, Some(read.id()));
    assert!(client
        .session(&session())
        .expect("live facts")
        .entry(&entry_id(9))
        .is_none());
    client
        .read(connection, Command::Watch {})
        .expect("the read race never closes a healthy connection");
}

#[test]
fn queued_text_replies_match_the_original_entry_revision_offset_and_request_id() {
    let (mut client, connection) = attached();
    let revision: carlos_proto::ids::QueueRevision =
        "b".repeat(64).parse().expect("queue revision");
    let queue_entry: carlos_proto::ids::QueueEntryId = "c".repeat(64).parse().expect("queue entry");
    let request = || Command::QueueText {
        session: session(),
        expected: revision.clone(),
        entry: queue_entry.clone(),
        offset: ByteOffset::new(2),
        limit: Some(4),
    };
    let first = client
        .read(connection, request())
        .expect("selected queued text");
    let wrong = json!({"session":session(),"revision":revision,"entry":"d".repeat(64),"offset":"0","next_offset":"4","complete":false,"text":"🌍"});
    assert!(matches!(
        client.receive(
            connection,
            Incoming::Success {
                id: first.id(),
                result: wrong
            }
        ),
        Err(ClientError::InvalidBody)
    ));
    let second = client
        .read(connection, request())
        .expect("read remains usable");
    let read = reply(
        &mut client,
        &second,
        json!({"session":session(),"revision":revision,"entry":queue_entry,"offset":"0","next_offset":"4","complete":false,"text":"🌍"}),
    );
    assert_eq!(read.replies[0].id, Some(second.id()));
    assert!(matches!(&read.replies[0].reply, ReadReply::QueueText(chunk) if chunk.text == "🌍"));
}

fn append(
    client: &mut crate::Client,
    connection: crate::ConnectionId,
    seq: u64,
    text: &str,
) -> crate::Effects {
    notify(
        client,
        connection,
        Notification::SessionEvent(event(
            seq,
            Event::TextAppended {
                entry: entry_id(10),
                text: text.into(),
            },
        )),
    )
}
