//! Distinguish accepted Starting facts from retained session copies while callbacks wait.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::session::Availability;
use serde_json::json;

use super::client::{attached, greet, notify, reply};
use super::{event, session, snapshot};
use crate::codec::Incoming;

#[test]
fn an_accepted_starting_snapshot_does_not_make_ordinary_sends_ready() {
    let (mut client, connection) = attached();
    let refresh = client
        .read(
            connection,
            Command::Subscribe {
                session: session(),
                since: None,
            },
        )
        .expect("explicit current snapshot");
    assert!(!client.session_has_current_snapshot(connection, &session()));
    let mut starting = snapshot();
    starting.info.availability = Availability::Starting;
    reply(
        &mut client,
        &refresh,
        json!({"kind":"snapshot","snapshot":starting}),
    );
    assert!(client.session_has_current_snapshot(connection, &session()));
    assert!(client.current_session_has_snapshot(&session()));
    assert!(!client.session_is_ready(connection, &session()));
}

#[test]
fn a_held_or_failed_resumed_subscribe_cannot_grant_callback_write_rights() {
    let (mut client, old) = attached();
    client.disconnect(old);
    let (connection, welcome) = greet(&mut client, true, "d");
    assert!(!client.session_has_current_snapshot(connection, &session()));
    assert!(!client.session_has_current_snapshot(old, &session()));
    client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(welcome.outgoing[0].id()),
                error: RpcError {
                    code: -32012,
                    message: "restore rejected".into(),
                },
            },
        )
        .expect("failed resumed subscribe");
    let mut info = client
        .session(&session())
        .expect("retained Ready copy")
        .info()
        .clone();
    info.availability = Availability::Starting;
    notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(11, Event::InfoChanged { info })),
    );
    assert!(
        !client.current_session_has_snapshot(&session()),
        "a fresh info event is not accepted subscription rights"
    );
    let retry = client
        .read(
            connection,
            Command::Subscribe {
                session: session(),
                since: None,
            },
        )
        .expect("explicit retry");
    let mut current = snapshot();
    current.seq = Sequence::new(11);
    current.info.availability = Availability::Starting;
    reply(
        &mut client,
        &retry,
        json!({"kind":"snapshot","snapshot":current}),
    );
    assert!(client.session_has_current_snapshot(connection, &session()));
    assert!(!client.session_is_ready(connection, &session()));
}

#[test]
fn an_older_reply_or_sequence_gap_cannot_accept_a_callback_snapshot() {
    let (mut client, connection) = attached();
    let first = client
        .read(
            connection,
            Command::Subscribe {
                session: session(),
                since: None,
            },
        )
        .expect("first refresh");
    let second = client
        .read(
            connection,
            Command::Subscribe {
                session: session(),
                since: None,
            },
        )
        .expect("replacement refresh");
    reply(
        &mut client,
        &first,
        json!({"kind":"snapshot","snapshot":snapshot()}),
    );
    assert!(!client.current_session_has_snapshot(&session()));
    reply(
        &mut client,
        &second,
        json!({"kind":"snapshot","snapshot":snapshot()}),
    );
    assert!(client.current_session_has_snapshot(&session()));
    let info = client
        .session(&session())
        .expect("current facts")
        .info()
        .clone();
    notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(13, Event::InfoChanged { info })),
    );
    assert!(
        !client.current_session_has_snapshot(&session()),
        "a missing event retires current callback rights"
    );
}
