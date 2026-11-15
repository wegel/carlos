//! Keep unrecovered queue text and an edited prompt private before any new socket command.

use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;

use carlos_core::session::models::Role;
use carlos_proto::bytes::ByteLength;
use carlos_proto::ids::{Sequence, SessionId};
use carlos_proto::prompts::{PromptRecord, PromptSelection, PromptSource};
use carlos_proto::session::Snapshot;

use super::Device;
use crate::app::remote::transport::Transport;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::tests::render_fixture;

fn fixture() -> (Device, Transport, UnixStream) {
    let session: SessionId = "a".repeat(32).parse().expect("session");
    let mut row = render_fixture::row(Role::User, "original prompt").entry;
    row.source = None;
    row.text = "original prompt".into();
    let selection = PromptSelection {
        session: session.clone(),
        source: PromptSource::Human,
        record: Some(PromptRecord {
            id: row.id.clone(),
            entry: Some(row.id.clone()),
            source: PromptSource::Human,
            bytes: ByteLength::new(row.text.len() as u64),
        }),
        has_older: false,
        has_newer: false,
    };
    let (mut device, connection) = device_with_snapshot(Snapshot {
        info: info(session, false),
        entries: vec![row],
        approvals: Vec::new(),
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    });
    device.history.selection = Some(selection);
    device.history.rewind_draft = Some("before rewind".into());
    let (stream, peer) = UnixStream::pair().expect("local pipe");
    let mut link = Transport::from_stream(stream, &mut carlos_client::Client::default())
        .expect("nonblocking transport");
    link.connection = connection;
    (device, link, peer)
}

#[test]
fn unrecovered_queue_text_blocks_another_add_without_replacing_either_draft() {
    let (mut device, mut link, mut peer) = fixture();
    device.set_composer("new composer");
    device.view.codex_queue.recovery = Some("submitted first".into());
    let error = device
        .submit_queue(
            &mut link,
            "a second failed send must not replace recovery".into(),
        )
        .expect_err("original recovery must be handled first");
    assert!(error.to_string().contains("Finish or recover"));
    assert_eq!(device.view.input.lines().join("\n"), "new composer");
    assert_eq!(
        device.view.codex_queue.recovery.as_deref(),
        Some("submitted first")
    );
    assert!(device.queue.operations.is_empty());
    assert!(device.client.operations().iter().next().is_none());
    assert!(commands(&mut link, &mut peer).is_empty());
}

#[test]
fn unrecovered_queue_text_blocks_history_fork_without_clearing_the_edited_composer() {
    let (mut device, mut link, mut peer) = fixture();
    device.set_composer("edited previous prompt");
    device.view.codex_queue.recovery = Some("unrecovered input".into());
    let selection = device.history.selection.clone();
    let error = device
        .submit_rewind(Some(&mut link), "edited previous prompt".into())
        .expect_err("fork must not discard the unrecovered queue text");
    assert!(error.to_string().contains("Finish or recover"));
    assert_eq!(
        device.view.input.lines().join("\n"),
        "edited previous prompt"
    );
    assert_eq!(device.history.selection, selection);
    assert!(device.history.operation.is_none());
    assert_eq!(
        device.view.codex_queue.recovery.as_deref(),
        Some("unrecovered input")
    );
    assert!(device.client.operations().iter().next().is_none());
    assert!(commands(&mut link, &mut peer).is_empty());
}

#[test]
fn explicit_recovery_releases_the_history_guard_without_changing_its_captured_anchor() {
    let (mut device, mut link, mut peer) = fixture();
    device.view.codex_queue.recovery = Some("unrecovered input".into());
    let selected = device.history.selection.clone();
    device
        .recover_queue_draft()
        .expect("explicit recovery into empty composer");
    assert_eq!(device.view.input.lines().join("\n"), "unrecovered input");
    assert!(device.view.codex_queue.recovery.is_none());
    assert_eq!(device.history.selection, selected);
    device.set_composer("edited previous prompt");
    device
        .submit_rewind(Some(&mut link), "edited previous prompt".into())
        .expect("recovered text no longer blocks the chosen history edit");
    let requests = commands(&mut link, &mut peer);
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["method"], "session/rewind");
    assert_eq!(requests[0]["params"]["session"], device.session.to_string());
    assert_eq!(
        requests[0]["params"]["entry"],
        selected
            .expect("captured selection")
            .record
            .expect("prompt")
            .id
            .to_string()
    );
    assert_eq!(requests[0]["params"]["text"], "edited previous prompt");
    assert!(device.view.input.is_empty());
    assert!(device.history.operation.is_some());
}

fn commands(link: &mut Transport, peer: &mut UnixStream) -> Vec<serde_json::Value> {
    link.flush().expect("flush bounded local writes");
    peer.set_nonblocking(true)
        .expect("deterministic available bytes");
    let mut text = String::new();
    if let Err(error) = peer.read_to_string(&mut text) {
        assert_eq!(error.kind(), ErrorKind::WouldBlock);
    }
    text.lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("actual wire request"))
        .filter(|request| request["method"] != "hello")
        .collect()
}
