//! Preserve private composers and narrow stale-context labels while accepted session facts change.

use std::os::unix::net::UnixStream;

use carlos_client::codec::Incoming;
use carlos_client::{Client, ConnectionId};
use carlos_proto::events::{Event, Notification, SessionEvent};
use carlos_proto::ids::Sequence;
use carlos_proto::session::{Availability, Backend, Context, SessionInfo, Snapshot};
use carlos_proto::status::StatusFacts;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

use crate::app::remote::transport::Transport;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::remote::Device;

fn facts(backend: Backend, fresh: bool) -> SessionInfo {
    let mut facts = info("a".repeat(32).parse().expect("session"), false);
    facts.backend = backend;
    facts.context = Some(Context {
        used: if fresh { 43000 } else { 42000 },
        max: 100000,
    });
    facts.status = Some(Box::new(StatusFacts {
        context_fresh: fresh,
        active_reasoning: None,
        activity: None,
        callbacks_pending: 0,
        tasks_uncertain: false,
        ralph: Default::default(),
    }));
    facts
}

fn snapshot(info: SessionInfo, sequence: u64) -> Snapshot {
    Snapshot {
        info,
        entries: Vec::new(),
        approvals: Vec::new(),
        seq: Sequence::new(sequence),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    }
}

fn link(connection: ConnectionId) -> (Transport, UnixStream) {
    let (stream, peer) = UnixStream::pair().expect("local nonblocking transport");
    let mut link = Transport::from_stream(stream, &mut Client::default()).expect("transport pipe");
    link.connection = connection;
    (link, peer)
}

fn replace(
    device: &mut Device,
    link: &mut Transport,
    info: SessionInfo,
    sequence: u64,
    reset: bool,
) {
    let event = if reset {
        Event::Reset {
            snapshot: Box::new(snapshot(info, sequence)),
        }
    } else {
        Event::InfoChanged { info }
    };
    device
        .receive(
            link,
            Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                session: device.session.clone(),
                seq: Sequence::new(sequence),
                event,
            }))),
        )
        .expect("actual device accepts current facts");
}

fn draw(device: &mut Device, width: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, 12)).expect("phone terminal");
    terminal
        .draw(|frame| device.draw(frame))
        .expect("actual device draw");
    terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

#[test]
fn phone_status_marks_stale_counts_then_redraws_fresh_native_values() {
    for width in [36, 18] {
        let (mut device, connection) =
            device_with_snapshot(snapshot(facts(Backend::Claude, false), 1));
        let (mut link, _peer) = link(connection);
        let stale = draw(&mut device, width);
        assert!(stale.contains('~'), "{width}: {stale}");
        if width == 36 {
            assert!(stale.contains("42k/100k"), "{stale}");
        }
        replace(
            &mut device,
            &mut link,
            facts(Backend::Claude, true),
            2,
            false,
        );
        let fresh = draw(&mut device, width);
        assert!(!fresh.contains('~'), "{width}: {fresh}");
        if width == 36 {
            assert!(fresh.contains("43k/100k"), "{fresh}");
        }
        assert!(device.client.operations().iter().next().is_none());
    }
}

#[test]
fn snapshot_replacement_and_shared_disconnect_preserve_the_exact_private_composer() {
    let (mut device, connection) = device_with_snapshot(snapshot(facts(Backend::Codex, true), 1));
    let (mut link, _peer) = link(connection);
    device.set_composer("preserve main 日本語");
    for (sequence, reset, availability) in [
        (2, true, Availability::Ready),
        (3, false, Availability::Closed),
        (4, true, Availability::Ready),
        (5, false, Availability::Closed),
        (6, true, Availability::Ready),
    ] {
        let mut info = facts(Backend::Codex, true);
        info.availability = availability;
        replace(&mut device, &mut link, info, sequence, reset);
        draw(&mut device, 36);
        assert_eq!(device.view.input.lines().join("\n"), "preserve main 日本語");
        assert!(device.client.operations().iter().next().is_none());
    }
}
