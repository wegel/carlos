//! Require a successful current-connection subscription before trusting retained Ready session facts.

use carlos_proto::commands::{Command, RpcError};
use carlos_proto::events::{Event, Notification};
use carlos_proto::ids::Sequence;
use carlos_proto::session::Availability;
use serde_json::json;

use super::client::{attached, greet, notify, reply};
use super::{event, session, snapshot};
use crate::codec::Incoming;

#[test]
fn failed_resumed_subscription_stays_blocked_until_an_explicit_successful_retry() {
    let (mut client, old) = attached();
    assert!(client.session_is_ready(old, &session()));
    client.disconnect(old);
    let (connection, welcome) = greet(&mut client, true, "d");
    assert!(!client.session_is_ready(connection, &session()));
    assert_eq!(
        client
            .session(&session())
            .expect("retained facts")
            .info()
            .availability,
        Availability::Ready
    );
    let _ = client
        .receive(
            connection,
            Incoming::Failure {
                id: Some(welcome.outgoing[0].id()),
                error: RpcError {
                    code: -32012,
                    message: "subscription rejected".into(),
                },
            },
        )
        .expect("failed subscribe");
    assert!(!client.current_session_is_ready(&session()));
    let info = client
        .session(&session())
        .expect("old ready copy")
        .info()
        .clone();
    let _ = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(11, Event::InfoChanged { info })),
    );
    assert!(
        !client.current_session_is_ready(&session()),
        "a later event cannot stand in for accepted subscribe rights"
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
    let _ = reply(
        &mut client,
        &retry,
        json!({"kind":"snapshot", "snapshot":current}),
    );
    assert!(client.session_is_ready(connection, &session()));
    assert!(!client.session_is_ready(old, &session()));
}

#[test]
fn superseded_subscription_reply_cannot_accept_a_newer_pending_subscription() {
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
        .expect("newer refresh");
    let _ = reply(
        &mut client,
        &first,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    assert!(!client.current_session_is_ready(&session()));
    let _ = reply(
        &mut client,
        &second,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    assert!(client.current_session_is_ready(&session()));
}
