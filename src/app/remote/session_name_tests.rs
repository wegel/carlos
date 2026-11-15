//! Draw accepted session names on the real device while keeping its composer private.

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_proto::events::{Event, Notification, SessionEvent};
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::session::{SessionInfo, Snapshot};
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Modifier;
use ratatui::Terminal;

use super::widget_fixture::{device_with_info, device_with_snapshot, info};
use super::Device;
use crate::theme::COLOR_STEP8;

fn named_info(name: &str) -> SessionInfo {
    let id = "a".repeat(32).parse().expect("session address");
    let mut metadata = info(id, false);
    metadata.name = Some(name.into());
    metadata
}

fn named(name: &str) -> Device {
    device_with_info(Vec::new(), named_info(name))
}

fn snapshot(name: &str) -> Snapshot {
    Snapshot {
        info: named_info(name),
        entries: Vec::new(),
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    }
}

fn render(device: &mut Device, width: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(width, 16)).expect("terminal");
    terminal
        .draw(|frame| device.draw(frame))
        .expect("draw title");
    terminal.backend().buffer().clone()
}

#[test]
fn name_placeholder_does_not_change_input_layout_cursor_or_history() {
    let mut device = named("Cleanup monitor");
    let buffer = render(&mut device, 80);
    assert_eq!(buffer[(2, 15)].symbol(), " ");
    assert_eq!(buffer[(64, 15)].symbol(), "C");
    assert_eq!(buffer[(78, 15)].symbol(), "r");
    assert_eq!(buffer[(79, 15)].symbol(), " ");
    assert_eq!(buffer[(64, 15)].fg, COLOR_STEP8);
    assert!(!buffer[(64, 15)].modifier.contains(Modifier::DIM));
    assert!(device.view.input.is_empty());
    assert_eq!(device.view.input.cursor(), (0, 0));
    assert!(device.client.operations().iter().next().is_none());
    assert!(device
        .client
        .session(&device.session)
        .expect("accepted session")
        .entries()
        .next()
        .is_none());
    assert!(device.history.selection.is_none());
    assert!(device.history.navigation_draft.is_none());
    device.set_composer("x");
    let buffer = render(&mut device, 80);
    assert_eq!(buffer[(1, 15)].symbol(), "x");
    assert!(!buffer[(2, 15)].modifier.contains(Modifier::DIM));
}

#[test]
fn right_aligned_names_clip_at_whole_unicode_characters_when_resizing() {
    let mut device = named("漢🦀x");
    let buffer = render(&mut device, 20);
    assert_eq!(buffer[(14, 15)].symbol(), "漢");
    assert_eq!(buffer[(16, 15)].symbol(), "🦀");
    assert_eq!(buffer[(18, 15)].symbol(), "x");
    assert_eq!(buffer[(19, 15)].symbol(), " ");
    let buffer = render(&mut device, 7);
    assert_eq!(buffer[(1, 15)].symbol(), "漢");
    assert_eq!(buffer[(3, 15)].symbol(), "🦀");
    assert_eq!(buffer[(5, 15)].symbol(), "x");
    assert_eq!(buffer[(6, 15)].symbol(), " ");
    let buffer = render(&mut device, 6);
    assert_eq!(buffer[(1, 15)].symbol(), "漢");
    assert_eq!(buffer[(3, 15)].symbol(), "🦀");
    assert_eq!(buffer[(5, 15)].symbol(), " ");
    let buffer = render(&mut device, 4);
    assert_eq!(buffer[(1, 15)].symbol(), "漢");
    assert_eq!(buffer[(3, 15)].symbol(), " ");
    assert!(device.view.input.is_empty());
}

#[test]
fn foreign_name_events_cannot_change_the_title_or_draft() {
    let (mut device, connection) = device_with_snapshot(snapshot("Original"));
    device.set_composer("keep my draft");
    let foreign = "e".repeat(32).parse().expect("foreign address");
    for (id, expected) in [
        (foreign, "Original"),
        (device.session.clone(), "External rename"),
    ] {
        accept_name(&mut device, connection, id, 2, Some("External rename"));
        assert_eq!(
            device
                .client
                .session(&device.session)
                .expect("accepted session")
                .info()
                .name
                .as_deref(),
            Some(expected)
        );
        assert_eq!(device.view.input.lines(), &["keep my draft"]);
    }
    let id = device.session.clone();
    accept_name(&mut device, connection, id, 3, None);
    assert!(device
        .client
        .session(&device.session)
        .expect("accepted session")
        .info()
        .name
        .is_none());
    assert_eq!(device.view.input.lines(), &["keep my draft"]);
    assert!(device.client.operations().iter().next().is_none());
}

fn accept_name(
    device: &mut Device,
    connection: ConnectionId,
    session: SessionId,
    seq: u64,
    name: Option<&str>,
) {
    let mut metadata = device
        .client
        .session(&device.session)
        .expect("accepted session")
        .info()
        .clone();
    metadata.id = session.clone();
    metadata.name = name.map(str::to_string);
    device
        .client
        .receive(
            connection,
            Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                session,
                seq: Sequence::new(seq),
                event: Event::InfoChanged { info: metadata },
            }))),
        )
        .expect("accepted numbered metadata");
}

#[test]
fn unicode_and_external_control_characters_cannot_expand_the_composer() {
    // Core's native metadata case proves the controls become this exact safe public name.
    let mut device = named("Résumé 漢字 🦀 second line");
    for width in [1, 3, 12, 80] {
        let _ = render(&mut device, width);
        assert!(device.view.input.is_empty());
        assert!(device.client.operations().iter().next().is_none());
    }
}
