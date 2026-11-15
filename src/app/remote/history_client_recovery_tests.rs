//! Accept correlated recovery only on the real greeted client that submitted its text.

use carlos_client::codec::decode;
use carlos_client::{ClientError, ConnectionId};
use carlos_proto::commands::{Command, RpcVersion, SendPolicy};
use carlos_proto::events::{Notification, NotificationMessage};
use carlos_proto::ids::Sequence;
use carlos_proto::session::{Backend, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, SubmittedText};

use super::{recover, transport};
use crate::app::remote::widget_fixture::{device_with_snapshot_for_client, info};
use crate::app::remote::Device;

fn device(client: char) -> (Device, ConnectionId) {
    let mut metadata = info("a".repeat(32).parse().expect("session"), false);
    metadata.backend = Backend::Claude;
    device_with_snapshot_for_client(
        Snapshot {
            info: metadata,
            entries: Vec::new(),
            approvals: Vec::new(),
            seq: Sequence::new(1),
            has_older: false,
            goal: None,
            queue: None,
            settings: None,
            tasks: None,
        },
        client
            .to_string()
            .repeat(32)
            .parse()
            .expect("distinct greeted client"),
    )
}

fn pending(
    device: &mut Device,
    connection: ConnectionId,
    text: &str,
    edit: bool,
) -> ClientOperation {
    let session = device.session.clone();
    let mut owner = None;
    device
        .client
        .submit(connection, |operation| {
            owner = Some(operation.clone());
            if edit {
                Command::Rewind {
                    session,
                    operation,
                    entry: format!("{}-{:016x}", "a".repeat(32), 1)
                        .parse()
                        .expect("prompt"),
                    text: text.into(),
                }
            } else {
                Command::Send {
                    session,
                    operation,
                    text: text.into(),
                    policy: SendPolicy::Queue,
                }
            }
        })
        .expect("actual client owns the original private text");
    let operation = owner.expect("captured submitting client");
    if edit {
        device.history.operation = Some(operation.clone());
    }
    operation
}

fn recovery(device: &Device, operation: &ClientOperation, text: &str) -> Vec<u8> {
    serde_json::to_vec(&NotificationMessage {
        jsonrpc: RpcVersion::V2,
        notification: Notification::Operation(OperationResult {
            operation: operation.clone(),
            session: Some(device.session.clone()),
            rewound: false,
            state: OperationState::Rejected {
                message: "failed fork".into(),
                recovery: Some(SubmittedText {
                    operation: Some(operation.clone()),
                    text: text.into(),
                }),
            },
        }),
    })
    .expect("same correlated private outcome bytes")
}

#[test]
fn two_clients_apply_the_same_correlated_recovery_without_overwriting_newer_text() {
    let (mut first, first_connection) = device('c');
    let (mut second, second_connection) = device('f');
    let origin = pending(&mut first, first_connection, "submitted edit", true);
    let foreign = pending(
        &mut second,
        second_connection,
        "foreign pending text",
        false,
    );
    assert_ne!(origin.client, foreign.client);
    first.set_composer("typed ahead");
    second.set_composer("second device draft");
    let bytes = recovery(&first, &origin, "submitted edit");
    let (mut first_link, _first_peer) = transport(first_connection);
    let (mut second_link, _second_peer) = transport(second_connection);
    first
        .receive(&mut first_link, decode(&bytes).expect("origin codec"))
        .expect("origin accepts its exact private result");
    let error = second
        .receive(&mut second_link, decode(&bytes).expect("foreign codec"))
        .expect_err("a private result cannot enter another client");
    assert!(matches!(
        error.downcast_ref::<ClientError>(),
        Some(ClientError::InvalidConnection)
    ));
    assert_eq!(first.view.input.lines().join("\n"), "typed ahead");
    assert_eq!(second.view.input.lines().join("\n"), "second device draft");
    assert_eq!(
        first.history.recoveries.iter().collect::<Vec<_>>(),
        [&origin]
    );
    assert!(second.history.recoveries.is_empty());
    assert_eq!(second.client.operations().iter().count(), 1);
    let untouched = second
        .client
        .operations()
        .get(&foreign)
        .expect("foreign record unchanged");
    assert_eq!(untouched.text(), Some("foreign pending text"));
    assert!(matches!(untouched.state, OperationState::Pending));
    first.set_composer("");
    recover(&mut first);
    assert_eq!(first.view.input.lines().join("\n"), "submitted edit");
}

#[test]
fn empty_originating_editor_recovers_but_empty_other_editor_stays_empty() {
    let (mut first, first_connection) = device('c');
    let (mut second, second_connection) = device('f');
    let origin = pending(&mut first, first_connection, "submitted", false);
    let foreign = pending(
        &mut second,
        second_connection,
        "foreign pending text",
        false,
    );
    assert_ne!(origin.client, foreign.client);
    let bytes = recovery(&first, &origin, "submitted");
    let (mut first_link, _first_peer) = transport(first_connection);
    let (mut second_link, _second_peer) = transport(second_connection);
    first
        .receive(&mut first_link, decode(&bytes).expect("origin codec"))
        .expect("empty origin recovers its own text");
    let error = second
        .receive(&mut second_link, decode(&bytes).expect("foreign codec"))
        .expect_err("a private result cannot enter another client");
    assert!(matches!(
        error.downcast_ref::<ClientError>(),
        Some(ClientError::InvalidConnection)
    ));
    assert_eq!(first.view.input.lines().join("\n"), "submitted");
    assert!(second.view.input.is_empty());
    assert!(first.history.recoveries.is_empty());
    assert!(second.history.recoveries.is_empty());
    assert!(first.client.operations().get(&origin).is_none());
    assert_eq!(second.client.operations().iter().count(), 1);
    let untouched = second
        .client
        .operations()
        .get(&foreign)
        .expect("foreign record unchanged");
    assert_eq!(untouched.text(), Some("foreign pending text"));
    assert!(matches!(untouched.state, OperationState::Pending));
}
