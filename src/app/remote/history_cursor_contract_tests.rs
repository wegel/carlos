//! Drive fixed typed prompt replies through the live device while retaining its private composer.

use std::os::unix::net::UnixStream;

use carlos_client::codec::Incoming;
use carlos_core::session::models::Role;
use carlos_proto::bytes::{ByteLength, ByteOffset};
use carlos_proto::events::{Event as SharedEvent, Notification, SessionEvent};
use carlos_proto::ids::Sequence;
use carlos_proto::prompts::{PromptRecord, PromptSelection, PromptSource, PromptText};
use carlos_proto::session::Snapshot;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::Device;
use crate::app::remote::transport::Transport;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::tests::render_fixture;

fn fixture(texts: &[&str]) -> (Device, Transport, UnixStream) {
    let session = "a".repeat(32).parse().expect("session");
    let entries = texts
        .iter()
        .map(|text| {
            let mut entry = render_fixture::row(Role::User, *text).entry;
            entry.source = None;
            entry.text = (*text).into();
            entry
        })
        .collect();
    let (device, connection) = device_with_snapshot(Snapshot {
        info: info(session, false),
        entries,
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    });
    let (stream, peer) = UnixStream::pair().expect("local pipe");
    let mut link = Transport::from_stream(stream, &mut carlos_client::Client::default())
        .expect("nonblocking transport");
    link.connection = connection;
    (device, link, peer)
}

fn reply(device: &mut Device, link: &mut Transport, value: serde_json::Value) {
    let id = device
        .history
        .pending
        .clone()
        .expect("exact current prompt read");
    device
        .receive(link, Incoming::Success { id, result: value })
        .expect("accepted prompt reply");
}

fn selected(device: &Device, index: Option<usize>) -> PromptSelection {
    let replica = device
        .client
        .session(&device.session)
        .expect("accepted entries");
    let record = index.map(|index| {
        let entry = replica.entry_at(index).expect("fixed prompt");
        PromptRecord {
            id: entry.id.clone(),
            entry: Some(entry.id.clone()),
            source: PromptSource::Human,
            bytes: ByteLength::new(entry.text.len() as u64),
        }
    });
    PromptSelection {
        session: device.session.clone(),
        source: PromptSource::Human,
        record,
        has_older: index.is_some_and(|i| i > 0),
        has_newer: index.is_some_and(|i| i + 1 < replica.entry_count()),
    }
}

fn navigate(device: &mut Device, link: &mut Transport, older: bool, index: Option<usize>) {
    device
        .navigate_prompt(Some(link), older)
        .expect("history key");
    if device.history.pending.is_none() {
        assert!(index.is_none(), "only unselected Down skips a native read");
        return;
    }
    let selection = selected(device, index);
    let record = selection.record.clone();
    reply(
        device,
        link,
        serde_json::to_value(selection).expect("typed selection"),
    );
    if let Some(record) = record {
        let text = device
            .client
            .session(&device.session)
            .expect("entries")
            .entry(record.entry.as_ref().expect("linked row"))
            .expect("prompt")
            .text
            .clone();
        let session = device.session.clone();
        reply(
            device,
            link,
            serde_json::to_value(PromptText {
                session,
                prompt: record.id,
                offset: ByteOffset::new(0),
                total: record.bytes,
                text,
                next: None,
            })
            .expect("typed original text"),
        );
    }
}

#[test]
fn navigation_clamps_at_oldest_and_restores_the_first_draft_at_newest() {
    let (mut device, mut link, _peer) = fixture(&["first", "second"]);
    device.set_composer("draft 日本語");
    for (up, text, index) in [
        (false, "draft 日本語", None),
        (true, "second", Some(1)),
        (true, "first", Some(0)),
        (true, "first", Some(0)),
        (false, "second", Some(1)),
        (false, "draft 日本語", None),
        (false, "draft 日本語", None),
    ] {
        navigate(&mut device, &mut link, up, index);
        assert_eq!(device.view.input.lines().join("\n"), text);
        assert_eq!(
            device
                .history
                .selection
                .as_ref()
                .and_then(|s| s.record.as_ref())
                .map(|r| &r.id),
            selected(&device, index).record.as_ref().map(|r| &r.id)
        );
    }
    let (mut empty, mut link, _peer) = fixture(&[]);
    empty.set_composer("unsent");
    navigate(&mut empty, &mut link, true, None);
    navigate(&mut empty, &mut link, false, None);
    assert_eq!(empty.view.input.lines().join("\n"), "unsent");
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn rewind_restores_its_original_draft_and_keeps_the_captured_anchor_until_escape() {
    let (mut device, mut link, _peer) = fixture(&["first", "second"]);
    device.set_composer("draft 日本語");
    device.history.rewind_draft = Some("draft 日本語".into());
    navigate(&mut device, &mut link, true, Some(1));
    let selection = device.history.selection.clone();
    device
        .event(None, Event::Paste(" edited second".into()))
        .expect("edit draft");
    assert_eq!(device.view.input.lines().join("\n"), "second edited second");
    assert_eq!(device.history.selection, selection);
    device
        .event(
            None,
            Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        )
        .expect("cancel rewind");
    assert_eq!(device.view.input.lines().join("\n"), "draft 日本語");
    assert!(device.history.selection.is_none());
    assert!(device.history.rewind_draft.is_none());
    navigate(&mut device, &mut link, true, Some(1));
    assert_eq!(device.view.input.lines().join("\n"), "second");
    navigate(&mut device, &mut link, false, None);
    assert_eq!(device.view.input.lines().join("\n"), "draft 日本語");
    assert!(device.client.operations().iter().next().is_none());
}

#[test]
fn replacing_history_during_rewind_resets_focus_but_preserves_the_composer() {
    for text in ["", "new prompt"] {
        let (mut device, mut link, _peer) = fixture(&["first", "second"]);
        device.history.rewind_draft = Some("draft 日本語".into());
        navigate(&mut device, &mut link, true, Some(1));
        device.set_composer("edit remains");
        replace_history(&mut device, &mut link, text);
        assert!(device.history.rewind_draft.is_none());
        assert!(device.history.selection.is_none());
        assert_eq!(device.view.input.lines().join("\n"), "edit remains");
        navigate(
            &mut device,
            &mut link,
            true,
            (!text.is_empty()).then_some(0),
        );
        assert_eq!(
            device.view.input.lines().join("\n"),
            if text.is_empty() {
                "edit remains"
            } else {
                text
            }
        );
        if !text.is_empty() {
            navigate(&mut device, &mut link, false, None);
            assert_eq!(device.view.input.lines().join("\n"), "edit remains");
        }
    }
}

fn replace_history(device: &mut Device, link: &mut Transport, text: &str) {
    let entries = if text.is_empty() {
        Vec::new()
    } else {
        let mut entry = render_fixture::row(Role::User, text).entry;
        entry.source = None;
        entry.text = text.into();
        vec![entry]
    };
    let snapshot = Snapshot {
        info: device
            .client
            .session(&device.session)
            .expect("session")
            .info()
            .clone(),
        entries,
        approvals: Vec::new(),
        seq: Sequence::new(2),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    };
    device
        .receive(
            link,
            Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                session: device.session.clone(),
                seq: Sequence::new(2),
                event: SharedEvent::Reset {
                    snapshot: Box::new(snapshot),
                },
            }))),
        )
        .expect("accepted replacement");
}

#[test]
fn reset_retires_pending_prompt_text_without_losing_private_work() {
    let (mut device, mut link, _peer) = fixture(&["first", "second"]);
    device.history.rewind_draft = Some("prior private draft".into());
    navigate(&mut device, &mut link, true, Some(1));
    device.set_composer("keep current edit");
    let (id, text) = pending_older_text(&mut device, &mut link);
    let pending = operation(1);
    let recovery = operation(2);
    device.history.operation = Some(pending.clone());
    device.history.recoveries.push_back(recovery.clone());
    device.history.failed = true;
    replace_history(&mut device, &mut link, "new prompt");
    assert!(device.history.pending.is_none());
    assert!(device.history.selection.is_none());
    assert!(device.history.rewind_draft.is_none());
    device
        .receive(
            &mut link,
            Incoming::Success {
                id,
                result: serde_json::to_value(text).expect("late original reply"),
            },
        )
        .expect("retired prompt reply stays harmless");
    assert_eq!(device.view.input.lines().join("\n"), "keep current edit");
    assert!(device.history.selection.is_none());
    assert_eq!(device.history.operation, Some(pending));
    assert_eq!(
        device
            .history
            .recoveries
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        [recovery]
    );
    assert!(device.history.failed);
}

fn pending_older_text(
    device: &mut Device,
    link: &mut Transport,
) -> (carlos_proto::commands::RequestId, PromptText) {
    device
        .navigate_prompt(Some(&mut *link), true)
        .expect("read older selection");
    let selection = selected(device, Some(0));
    let record = selection.record.clone().expect("original record");
    let text = device
        .client
        .session(&device.session)
        .expect("entries")
        .entry(record.entry.as_ref().expect("linked row"))
        .expect("first prompt")
        .text
        .clone();
    reply(
        device,
        link,
        serde_json::to_value(selection).expect("typed selection"),
    );
    let id = device
        .history
        .pending
        .clone()
        .expect("pending original text");
    (
        id,
        PromptText {
            session: device.session.clone(),
            prompt: record.id,
            offset: ByteOffset::new(0),
            total: record.bytes,
            text,
            next: None,
        },
    )
}

fn operation(counter: u64) -> carlos_proto::submissions::ClientOperation {
    carlos_proto::submissions::ClientOperation {
        client: "c".repeat(32).parse().expect("device client"),
        id: counter
            .to_string()
            .parse()
            .expect("captured private operation"),
    }
}
