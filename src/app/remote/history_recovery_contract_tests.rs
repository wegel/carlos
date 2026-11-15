//! Recover failed human text once without replacing this terminal's newer composer.

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_proto::commands::{Command, SendPolicy};
use carlos_proto::ids::Sequence;
use carlos_proto::session::{Backend, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, SubmittedText};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

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
