//! Paint edit progress above a retained older prompt on both recorded phone widths.

use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_core::session::models::Role;
use carlos_proto::bytes::ByteLength;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::events::{Event as SharedEvent, Notification, SessionEvent};
use carlos_proto::ids::Sequence;
use carlos_proto::prompts::{PromptRecord, PromptSelection, PromptSource};
use carlos_proto::session::{Availability, Backend, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, SubmittedText};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::remote::transport::Transport;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::remote::Device;
use crate::app::tests::render_fixture;

fn device() -> (Device, ConnectionId, Transport, UnixStream) {
    let mut info = info("a".repeat(32).parse().expect("session"), false);
    info.backend = Backend::Claude;
    let rows = std::iter::once((Role::User, "original prompt".into()))
        .chain((0..80).map(|index| (Role::Assistant, format!("Retained row {index}"))))
        .map(|(role, text)| {
            let mut entry = render_fixture::row(role, &text).entry;
            entry.source = None;
            entry.text = text;
            entry
        })
        .collect::<Vec<_>>();
    let selected = rows[0].id.clone();
    let (mut device, connection) = device_with_snapshot(Snapshot {
        info,
        entries: rows,
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    });
    device.history.selection = Some(PromptSelection {
        session: device.session.clone(),
        source: PromptSource::Human,
        record: Some(PromptRecord {
            id: selected.clone(),
            entry: Some(selected),
            source: PromptSource::Human,
            bytes: ByteLength::new(15),
        }),
        has_older: false,
        has_newer: true,
    });
    device.history.rewind_draft = Some("before rewind".into());
    device.view.viewport.auto_follow_bottom = false;
    let (stream, peer) = UnixStream::pair().expect("local socket");
    let mut link = Transport::from_stream(stream, &mut carlos_client::Client::default())
        .expect("nonblocking device transport");
    link.connection = connection;
    (device, connection, link, peer)
}

fn begin(device: &mut Device, connection: ConnectionId) -> (RequestId, ClientOperation) {
    let session = device.session.clone();
    let entry = device
        .history
        .selection
        .as_ref()
        .expect("selected prompt")
        .record
        .as_ref()
        .expect("record")
        .id
        .clone();
    let mut owner = None;
    let request = device
        .client
        .submit(connection, |operation| {
            owner = Some(operation.clone());
            Command::Rewind {
                session,
                operation,
                entry,
                text: "edited".into(),
            }
        })
        .expect("typed edited prompt request");
    let operation = owner.expect("private operation");
    device.history.operation = Some(operation.clone());
    device.history.failed = false;
    device.view.status_hint = None;
    device.set_composer("");
    (request.id(), operation)
}

fn fail(device: &mut Device, link: &mut Transport, id: RequestId, operation: ClientOperation) {
    let result = OperationResult {
        operation: operation.clone(),
        session: Some(device.session.clone()),
        rewound: false,
        state: OperationState::Rejected {
            message: "source unavailable".into(),
            recovery: Some(SubmittedText {
                operation: Some(operation),
                text: "edited".into(),
            }),
        },
    };
    device
        .receive(
            link,
            Incoming::Success {
                id,
                result: serde_json::to_value(result).expect("private edit failure"),
            },
        )
        .expect("accepted current edit outcome");
}

fn draw(device: &mut Device, width: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).expect("phone terminal");
    terminal
        .draw(|frame| device.draw(frame))
        .expect("live device draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn phone_draws_progress_and_failure_even_when_an_older_rewind_row_is_selected() {
    let (mut device, connection, mut link, _peer) = device();
    let selected = device.history.selection.clone();
    for width in [36, 18] {
        let (id, operation) = begin(&mut device, connection);
        let waiting = draw(&mut device, width);
        assert!(
            waiting.contains(if width == 18 {
                "Editing"
            } else {
                "Preparing edited branch"
            }),
            "width {width}: {waiting}"
        );
        assert_eq!(device.history.selection, selected);
        assert_eq!(device.view.viewport.scroll_top, 0);
        fail(&mut device, &mut link, id, operation);
        let failed = draw(&mut device, width);
        assert!(failed.contains("Edit failed"), "width {width}: {failed}");
        assert_eq!(device.history.selection, selected);
        assert_eq!(device.view.viewport.scroll_top, 0);
        assert_eq!(device.view.input.lines().join("\n"), "edited");
        assert_eq!(
            device
                .client
                .session(&device.session)
                .expect("history")
                .entry_count(),
            81
        );
    }
}

#[test]
fn preparing_edit_ignores_three_more_enter_keys_and_keeps_the_newer_composer() {
    let (mut device, _, mut link, mut peer) = device();
    device.set_composer("edited");
    enter(&mut device, &mut link);
    let pending = device
        .history
        .operation
        .clone()
        .expect("first edit pending");
    let selected = device.history.selection.clone();
    assert_eq!(device.client.operations().iter().count(), 1);
    device.set_composer("newer unsent composer");
    for _ in 0..3 {
        enter(&mut device, &mut link);
        assert_eq!(device.history.operation.as_ref(), Some(&pending));
        assert_eq!(
            device.view.input.lines().join("\n"),
            "newer unsent composer"
        );
        assert_eq!(device.history.selection, selected);
        assert_eq!(device.client.operations().iter().count(), 1);
    }
    let requests = requests(&mut link, &mut peer);
    assert_eq!(requests.len(), 1, "duplicate Enter sends no extra command");
    assert_eq!(requests[0]["method"], "session/rewind");
    assert_eq!(requests[0]["params"]["text"], "edited");
}

#[test]
fn enter_during_initial_startup_keeps_the_rewind_composer_without_any_request() {
    let (mut device, _, mut link, mut peer) = device();
    let mut info = device
        .client
        .session(&device.session)
        .expect("session")
        .info()
        .clone();
    info.availability = Availability::Starting;
    device
        .receive(
            &mut link,
            Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                session: device.session.clone(),
                seq: Sequence::new(2),
                event: SharedEvent::InfoChanged { info },
            }))),
        )
        .expect("accepted native startup facts");
    let selected = device.history.selection.clone();
    device.set_composer("edit while starting");
    enter(&mut device, &mut link);
    assert_eq!(device.view.input.lines().join("\n"), "edit while starting");
    assert_eq!(device.history.selection, selected);
    assert!(device.history.operation.is_none());
    assert!(device.client.operations().iter().next().is_none());
    assert!(requests(&mut link, &mut peer).is_empty());
}

fn enter(device: &mut Device, link: &mut Transport) {
    device
        .event(
            Some(link),
            Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
        )
        .expect("actual Enter path");
}

fn requests(link: &mut Transport, peer: &mut UnixStream) -> Vec<serde_json::Value> {
    link.flush().expect("flush bounded local commands");
    peer.set_nonblocking(true)
        .expect("read available socket bytes");
    let mut text = String::new();
    if let Err(error) = peer.read_to_string(&mut text) {
        assert_eq!(error.kind(), ErrorKind::WouldBlock);
    }
    text.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("actual socket request"))
        .filter(|request| request["method"] != "hello")
        .collect()
}
