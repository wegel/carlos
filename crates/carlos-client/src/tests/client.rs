//! Exercise one complete device client through causal greetings, replies, gaps and private echoes.

use carlos_proto::commands::{Command, Request, RequestId};
use carlos_proto::events::{Event, Notification};
use carlos_proto::submissions::OperationState;
use serde_json::{json, Value};

use super::{entry, entry_id, event, session, snapshot};
use crate::codec::Incoming;
use crate::{Client, ClientError, ConnectionId, Effects, Outbound, Welcome};

pub(super) fn command(outgoing: &Outbound) -> Request {
    serde_json::from_slice(outgoing.bytes()).expect("typed outgoing command")
}

pub(super) fn reply(client: &mut Client, outgoing: &Outbound, value: Value) -> Effects {
    client
        .receive(
            outgoing.connection(),
            Incoming::Success {
                id: outgoing.id(),
                result: value,
            },
        )
        .expect("correlated reply")
}

pub(super) fn greet(client: &mut Client, resumed: bool, instance: &str) -> (ConnectionId, Effects) {
    let outgoing = client.begin("device".into()).expect("begin transport");
    let connection = outgoing.connection();
    let effects = reply(
        client,
        &outgoing,
        json!({"protocol":1,"host":"host","daemon_version":"test", "instance":instance.repeat(32),
        "client":"c".repeat(32), "resumed":resumed, "next_operation":"1", "resume_token":"e".repeat(64)}),
    );
    (connection, effects)
}

pub(super) fn subscribe(client: &mut Client, connection: ConnectionId) -> Outbound {
    client
        .read(
            connection,
            Command::Subscribe {
                session: session(),
                since: None,
            },
        )
        .expect("subscribe")
}

pub(super) fn attached() -> (Client, ConnectionId) {
    let mut client = Client::default();
    let (connection, welcome) = greet(&mut client, false, "d");
    assert_eq!(welcome.welcome, Some(Welcome::Fresh));
    let request = subscribe(&mut client, connection);
    let _ = reply(
        &mut client,
        &request,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    (client, connection)
}

pub(super) fn notify(
    client: &mut Client,
    connection: ConnectionId,
    notification: Notification,
) -> Effects {
    client
        .receive(connection, Incoming::Notification(Box::new(notification)))
        .expect("numbered notification")
}

#[test]
fn a_gap_requests_one_full_refresh_and_replacement_keeps_its_actual_cursor() {
    let (mut client, connection) = attached();
    let gap = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            12,
            Event::TextAppended {
                entry: entry_id(10),
                text: "gap".into(),
            },
        )),
    );
    assert!(client.session(&session()).expect("facts").is_stale());
    assert_eq!(gap.outgoing.len(), 1);
    assert!(matches!(
        command(&gap.outgoing[0]).command,
        Command::Subscribe { since: None, .. }
    ));
    let later = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            13,
            Event::TextAppended {
                entry: entry_id(10),
                text: "still stale".into(),
            },
        )),
    );
    assert!(later.outgoing.is_empty());
    let mut refreshed = snapshot();
    refreshed.seq = carlos_proto::ids::Sequence::new(13);
    refreshed.entries = vec![entry(10, "complete replacement")];
    let adopted = reply(
        &mut client,
        &gap.outgoing[0],
        json!({"kind":"snapshot", "snapshot":refreshed}),
    );
    assert!(adopted.outgoing.is_empty());
    assert!(!client
        .session(&session())
        .expect("current facts")
        .is_stale());
    assert_eq!(
        client
            .session(&session())
            .expect("current facts")
            .sequence()
            .get(),
        13
    );
    assert_row(&client, "complete replacement");
}

#[test]
fn authoritative_echo_and_private_acceptance_before_the_admission_reply_never_duplicate_a_prompt() {
    let (mut client, connection) = attached();
    let request = client
        .submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "exact prompt".into(),
            policy: Default::default(),
        })
        .expect("explicit send");
    let Command::Send { operation, .. } = command(&request).command else {
        panic!("send");
    };
    assert!(client
        .operations()
        .get(&operation)
        .expect("submitted text")
        .needs_echo());
    let accepted = json!({"operation":operation, "session":session(), "state":{"kind":"accepted", "entry":null}});
    let _ = notify(
        &mut client,
        connection,
        Notification::Operation(serde_json::from_value(accepted).expect("acceptance")),
    );
    let mut prompt = entry(11, "exact prompt");
    prompt.operation = Some(operation.clone());
    prompt.kind = carlos_proto::session::EntryKind::User;
    let changes = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(11, Event::EntryAdded { entry: prompt })),
    );
    assert_eq!(changes.operations, vec![operation.clone()]);
    assert!(!client
        .operations()
        .get(&operation)
        .expect("bound echo")
        .needs_echo());
    let _ = reply(
        &mut client,
        &request,
        json!({"operation":operation, "session":session(), "state":{"kind":"pending"}}),
    );
    assert!(matches!(
        client
            .operations()
            .get(&operation)
            .expect("private result")
            .state,
        OperationState::Accepted { .. }
    ));
    assert_eq!(
        client.session(&session()).expect("facts").entries().count(),
        2
    );
}

#[test]
fn a_late_superseded_subscription_reply_cannot_replace_the_current_session_window() {
    let (mut client, connection) = attached();
    let first = subscribe(&mut client, connection);
    let second = subscribe(&mut client, connection);
    let mut old = snapshot();
    old.seq = carlos_proto::ids::Sequence::new(100);
    old.entries[0].text = "superseded response".into();
    let _ = reply(
        &mut client,
        &first,
        json!({"kind":"snapshot", "snapshot":old}),
    );
    assert_eq!(
        client
            .session(&session())
            .expect("unchanged")
            .sequence()
            .get(),
        10
    );
    let _ = reply(
        &mut client,
        &second,
        json!({"kind":"snapshot", "snapshot":snapshot()}),
    );
    let duplicate = notify(
        &mut client,
        connection,
        Notification::SessionEvent(event(
            10,
            Event::TextAppended {
                entry: entry_id(10),
                text: "duplicate".into(),
            },
        )),
    );
    assert!(duplicate.sessions.is_empty());
    assert_eq!(
        client
            .session(&session())
            .expect("facts")
            .entry(&entry_id(10))
            .expect("row")
            .text,
        "hello"
    );
}

#[test]
fn private_notifications_reject_another_clients_origin_before_changing_local_text() {
    let (mut client, connection) = attached();
    let request = client
        .submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "owned draft".into(),
            policy: Default::default(),
        })
        .expect("send");
    let Command::Send { operation, .. } = command(&request).command else {
        panic!("send");
    };
    let foreign = serde_json::from_value(json!({"operation":{"client":"f".repeat(32), "id":operation.id}, "session":session(), "state":{"kind":"accepted", "entry":null}})).expect("foreign result");
    assert!(matches!(
        client.receive(
            connection,
            Incoming::Notification(Box::new(Notification::Operation(foreign)))
        ),
        Err(ClientError::InvalidConnection)
    ));
    assert_eq!(
        client
            .operations()
            .get(&operation)
            .expect("preserved draft")
            .text(),
        Some("owned draft")
    );
    assert!(matches!(
        client
            .operations()
            .get(&operation)
            .expect("preserved draft")
            .state,
        OperationState::Pending
    ));
}

#[test]
fn full_request_slots_reject_new_work_without_losing_its_original_submitted_text() {
    let (mut client, connection) = attached();
    let requests: Vec<_> = (0..128)
        .map(|_| {
            client
                .read(connection, Command::Watch {})
                .expect("request slot")
        })
        .collect();
    assert!(matches!(
        client.submit(connection, |operation| Command::Send {
            session: session(),
            operation,
            text: "keep exact draft".into(),
            policy: Default::default()
        }),
        Err(ClientError::Capacity(_))
    ));
    let retained = client
        .operations()
        .iter()
        .next()
        .expect("local failure retains its text");
    assert_eq!(retained.text(), Some("keep exact draft"));
    assert!(matches!(retained.state, OperationState::Rejected { .. }));
    let effects = reply(&mut client, &requests[0], json!({"sessions":[]}));
    assert!(
        effects.outgoing.is_empty(),
        "freeing a read slot never resends the rejected mutation"
    );
}

#[test]
fn unrelated_or_duplicate_request_ids_have_no_effect_on_checked_facts() {
    let (mut client, connection) = attached();
    let effects = client
        .receive(
            connection,
            Incoming::Success {
                id: RequestId::Integer(99999),
                result: json!({"arbitrary":"ignored"}),
            },
        )
        .expect("unissued response ignored");
    assert!(effects.sessions.is_empty());
    assert_eq!(
        client
            .session(&session())
            .expect("current facts")
            .sequence()
            .get(),
        10
    );
}

fn assert_row(client: &Client, text: &str) {
    assert_eq!(
        client
            .session(&session())
            .expect("current facts")
            .entry(&entry_id(10))
            .expect("row")
            .text,
        text
    );
}
