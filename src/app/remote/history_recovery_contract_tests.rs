//! Recover failed human text once without replacing this terminal's newer composer.

use std::os::unix::net::UnixStream;

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_proto::commands::{Command, HelloResult, SendPolicy};
use carlos_proto::ids::{OperationId, Sequence};
use carlos_proto::session::{Backend, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, SubmittedText};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use crate::app::remote::transport::Transport;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::remote::Device;

fn device() -> (Device, ConnectionId) {
    let mut metadata = info("a".repeat(32).parse().expect("session"), false);
    metadata.backend = Backend::Claude;
    device_with_snapshot(Snapshot {
        info: metadata,
        entries: Vec::new(),
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    })
}

fn reject(
    device: &mut Device,
    connection: ConnectionId,
    text: &str,
    edit: bool,
) -> ClientOperation {
    let session = device.session.clone();
    let mut origin = None;
    let outgoing = device
        .client
        .submit(connection, |operation| {
            origin = Some(operation.clone());
            if edit {
                Command::Rewind {
                    session,
                    operation,
                    entry: format!("{}-{:016x}", "a".repeat(32), 1)
                        .parse()
                        .expect("selected prompt"),
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
        .expect("explicit original text retained");
    let operation = origin.expect("captured origin");
    let result = OperationResult {
        operation: operation.clone(),
        session: Some(device.session.clone()),
        rewound: false,
        state: OperationState::Rejected {
            message: "startup failed".into(),
            recovery: Some(SubmittedText {
                operation: Some(operation.clone()),
                text: text.into(),
            }),
        },
    };
    let effects = device
        .client
        .receive(
            connection,
            Incoming::Success {
                id: outgoing.id(),
                result: serde_json::to_value(result).expect("private outcome"),
            },
        )
        .expect("matching private rejection");
    assert_eq!(
        effects.operations.as_slice(),
        std::slice::from_ref(&operation)
    );
    operation
}

fn recover(device: &mut Device) {
    device
        .event(
            None,
            Event::Key(KeyEvent::new(KeyCode::F(5), KeyModifiers::NONE)),
        )
        .expect("explicit F5 recovery");
}

#[test]
fn closed_startup_recovers_queued_humans_in_order_without_replacing_the_new_composer() {
    let (mut device, connection) = device();
    device.set_composer("newer draft");
    let first = reject(&mut device, connection, "first human", false);
    let second = reject(&mut device, connection, "second human", false);
    assert!(device.apply_failed_draft(&first));
    assert!(device.apply_failed_draft(&second));
    assert!(device.apply_failed_draft(&first));
    assert_eq!(device.view.input.lines().join("\n"), "newer draft");
    recover(&mut device);
    assert_eq!(device.view.input.lines().join("\n"), "newer draft");
    for expected in ["first human", "second human"] {
        device.set_composer("");
        recover(&mut device);
        assert_eq!(device.view.input.lines().join("\n"), expected);
    }
    device.set_composer("");
    recover(&mut device);
    assert!(device.view.input.is_empty());
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn a_failed_edit_is_recovered_once_without_overwriting_a_newer_composer() {
    let (mut device, connection) = device();
    device.set_composer("newer composer");
    let operation = reject(&mut device, connection, "edited before startup", true);
    device.history.operation = Some(operation.clone());
    assert!(device.apply_history_operation(&operation));
    assert!(device.history.failed);
    assert_eq!(device.view.input.lines().join("\n"), "newer composer");
    recover(&mut device);
    assert_eq!(device.view.input.lines().join("\n"), "newer composer");
    device.set_composer("");
    recover(&mut device);
    assert_eq!(
        device.view.input.lines().join("\n"),
        "edited before startup"
    );
    device.set_composer("");
    recover(&mut device);
    assert!(device.view.input.is_empty());
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn identical_failed_edit_text_stays_in_the_composer_without_a_duplicate_saved_draft() {
    for text in ["edited text", "edited 日本語\nsecond line\n"] {
        let (mut device, connection) = device();
        device.set_composer(text);
        let operation = reject(&mut device, connection, text, true);
        device.history.operation = Some(operation.clone());
        assert!(device.apply_history_operation(&operation));
        assert_eq!(device.view.input.lines().join("\n"), text);
        assert!(device.history.failed);
        assert_eq!(device.history_hint(36), Some("Edit failed · draft kept"));
        assert!(
            device.history.recoveries.is_empty(),
            "the edited text is already present"
        );
        assert!(device.client.operations().get(&operation).is_none());
        accept_unrelated(&mut device, connection);
        assert!(
            device.history.failed,
            "an unrelated acknowledgement cannot clear an edit failure"
        );
        assert_eq!(device.history_hint(18), Some("Edit failed"));
        assert_eq!(device.view.input.lines().join("\n"), text);
        device.set_composer("");
        recover(&mut device);
        assert!(
            device.view.input.is_empty(),
            "F5 cannot load a duplicate edit"
        );
    }
}

#[test]
fn identical_queued_human_text_keeps_its_separate_recovery_until_explicitly_loaded() {
    let (mut device, connection) = device();
    device.set_composer("identical queued human");
    let operation = reject(&mut device, connection, "identical queued human", false);
    assert!(device.apply_failed_draft(&operation));
    assert_eq!(
        device.view.input.lines().join("\n"),
        "identical queued human"
    );
    assert_eq!(
        device.history.recoveries.iter().collect::<Vec<_>>(),
        [&operation]
    );
    assert!(device.client.operations().get(&operation).is_some());
    device.set_composer("");
    recover(&mut device);
    assert_eq!(
        device.view.input.lines().join("\n"),
        "identical queued human"
    );
    assert!(device.history.recoveries.is_empty());
    assert!(device.client.operations().get(&operation).is_none());
}

fn accept_unrelated(device: &mut Device, connection: ConnectionId) {
    let session = device.session.clone();
    let mut owner = None;
    let request = device
        .client
        .submit(connection, |operation| {
            owner = Some(operation.clone());
            Command::SetRalph {
                session,
                operation,
                on: false,
                options: None,
            }
        })
        .expect("independent command");
    let result = OperationResult {
        operation: owner.expect("independent operation"),
        session: Some(device.session.clone()),
        rewound: false,
        state: OperationState::Accepted {
            entry: None,
            notice: None,
        },
    };
    let (mut link, _peer) = transport(connection);
    device
        .receive(
            &mut link,
            Incoming::Success {
                id: request.id(),
                result: serde_json::to_value(result).expect("unrelated private acceptance"),
            },
        )
        .expect("actual Device applies unrelated acknowledgement");
}

#[test]
fn a_replaced_greeting_keeps_an_identical_edit_once_and_retains_a_different_edit_separately() {
    for composer in ["edited text", "newer composer"] {
        let (mut device, connection) = device();
        device.set_composer(composer);
        let operation = pending_edit(&mut device, connection);
        device.client.disconnect(connection);
        let hello = device
            .client
            .begin("replacement".into())
            .expect("fresh transport");
        let (mut link, _peer) = transport(hello.connection());
        device
            .receive(
                &mut link,
                Incoming::Success {
                    id: hello.id(),
                    result: serde_json::to_value(replacement()).expect("replaced greeting"),
                },
            )
            .expect("actual replaced-greeting path");
        assert_eq!(device.view.input.lines().join("\n"), composer);
        assert!(device.history.failed);
        assert!(device.history.operation.is_none());
        if composer == "edited text" {
            assert!(
                device.history.recoveries.is_empty(),
                "visible edit text has no duplicate"
            );
            assert!(device.client.operations().get(&operation).is_none());
        } else {
            assert_eq!(
                device.history.recoveries.iter().collect::<Vec<_>>(),
                [&operation]
            );
            assert_eq!(
                device
                    .client
                    .operations()
                    .get(&operation)
                    .expect("retained edit")
                    .text(),
                Some("edited text")
            );
        }
    }
}

fn replacement() -> HelloResult {
    HelloResult {
        identity: None,
        protocol: carlos_proto::PROTOCOL_VERSION,
        host: "fixture".into(),
        daemon_version: "fixture".into(),
        instance: "e".repeat(32).parse().expect("new daemon"),
        client: "f".repeat(32).parse().expect("new lease"),
        resumed: false,
        next_operation: OperationId::new(1),
        resume_token: "9".repeat(64).parse().expect("new private token"),
    }
}

fn transport(connection: ConnectionId) -> (Transport, UnixStream) {
    let (stream, peer) = UnixStream::pair().expect("local socket");
    let mut link = Transport::from_stream(stream, &mut carlos_client::Client::default())
        .expect("nonblocking transport");
    link.connection = connection;
    (link, peer)
}

fn pending_edit(device: &mut Device, connection: ConnectionId) -> ClientOperation {
    let session = device.session.clone();
    let mut owner = None;
    device
        .client
        .submit(connection, |operation| {
            owner = Some(operation.clone());
            Command::Rewind {
                session,
                operation,
                entry: format!("{}-{:016x}", "a".repeat(32), 1)
                    .parse()
                    .expect("prompt"),
                text: "edited text".into(),
            }
        })
        .expect("pending original edit");
    let operation = owner.expect("edit origin");
    device.history.operation = Some(operation.clone());
    operation
}

#[path = "history_client_recovery_tests.rs"]
mod clients;

#[path = "history_send_recovery_tests.rs"]
mod sends;
