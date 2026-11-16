//! Charge complete and incomplete queue inputs to the shared device budget without losing drafts.

use carlos_proto::bytes::ByteLength;
use carlos_proto::commands::Command;
use carlos_proto::ids::SessionId;
use serde_json::json;

use super::client::{attached, command, reply};
use super::queue_panel::{page, single_read};
use super::{session, snapshot};
use crate::panels::QueueReadMode;
use crate::{Client, ConnectionId, Effects};

#[test]
fn input_cache_overflow_closes_only_its_reader_and_keeps_other_inputs_and_submitted_text() {
    let (mut client, connection) = attached();
    client
        .submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "private submitted text".into(),
            policy: Default::default(),
        })
        .expect("private operation");
    for index in 1..=4 {
        let session: SessionId = format!("{index:032x}")
            .parse()
            .expect("independent session");
        let read = input_reader(&mut client, connection, &session);
        let effects = read_whole_input(&mut client, &session, read);
        if index < 4 {
            assert!(effects.failures.is_empty());
            assert!(client.queue_panel(&session).is_some());
        } else {
            assert_eq!(effects.failures[0].error.code, -32010);
            assert!(!client.wants_session(&session));
            assert!(client.queue_panel(&session).is_none());
            assert!(effects
                .outgoing
                .iter()
                .all(|outgoing| matches!(command(outgoing).command, Command::Unsubscribe { .. })));
        }
    }
    for index in 1..=3 {
        let session = format!("{index:032x}").parse().expect("retained session");
        let input = client
            .take_queue_input(&session)
            .expect("another device window stays intact");
        assert_eq!(input.text.len(), 16 * 1024 * 1024);
    }
    assert_eq!(
        client
            .operations()
            .iter()
            .next()
            .expect("private text")
            .text(),
        Some("private submitted text")
    );
}

fn read_whole_input(client: &mut Client, session: &SessionId, mut effects: Effects) -> Effects {
    while !effects.outgoing.is_empty() && effects.failures.is_empty() {
        let outgoing = single_read(&effects);
        let Command::QueueText {
            expected,
            entry,
            offset,
            ..
        } = command(outgoing).command
        else {
            panic!("bounded input read")
        };
        let next = offset.get() + 65536;
        effects = reply(
            client,
            outgoing,
            json!({
                "session":session,"revision":expected,"entry":entry,"offset":offset,
                "next_offset":next.to_string(),"complete":next == 16 * 1024 * 1024,
                "text":"x".repeat(65536)
            }),
        );
    }
    effects
}

fn input_reader(client: &mut Client, connection: ConnectionId, session: &SessionId) -> Effects {
    let mut facts = snapshot();
    facts.info.id = session.clone();
    let subscribe = client
        .read(
            connection,
            Command::Subscribe {
                session: session.clone(),
                since: None,
            },
        )
        .expect("session reader");
    let _ = reply(
        client,
        &subscribe,
        json!({"kind":"snapshot","snapshot":facts}),
    );
    let opened = client
        .open_queue_panel(session, None, QueueReadMode::Refresh)
        .expect("open queue");
    let mut preview = page(0, 1, 10, 'a');
    preview.session = session.clone();
    preview.entries[0].text_bytes = Some(ByteLength::new(16 * 1024 * 1024));
    let _ = reply(client, single_read(&opened), json!(preview));
    client
        .begin_queue_input(session)
        .expect("read complete native input")
}
