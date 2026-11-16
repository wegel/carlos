//! Keep real devices' private drafts and confirmations attached to accepted opaque cards.

use carlos_client::{codec::Incoming, ConnectionId};
use carlos_proto::events::{Event, Notification, SessionEvent};
use carlos_proto::ids::{EntryId, Sequence};
use carlos_proto::session::{Approval, Backend, Phase, Snapshot};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::Device;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};

fn id(index: u64) -> EntryId {
    format!("{}-{index:016x}", "a".repeat(32))
        .parse()
        .expect("opaque card")
}

fn card(index: u64) -> Approval {
    serde_json::from_value(
        json!({"id":id(index),"kind":"question","title":"Original question",
        "details":["native details"],"choices":["once","deny","cancel"],
        "input":{"kind":"form","schema":{"type":"object","properties":{"value":{"type":"string"}}}},
        "active":index==1,"blocking":true,"submitted":false,"default_to_no":true}),
    )
    .expect("typed native card")
}

fn snapshot(backend: Backend, cards: Vec<Approval>, sequence: u64) -> Snapshot {
    let mut info = info("a".repeat(32).parse().expect("session"), false);
    info.backend = backend;
    info.phase = Phase::Approval;
    Snapshot {
        info,
        entries: Vec::new(),
        approvals: cards,
        seq: Sequence::new(sequence),
        has_older: false,
        goal: None,
        queue: None,
        tasks: None,
        settings: None,
    }
}

fn device(backend: Backend, cards: Vec<Approval>) -> (Device, ConnectionId) {
    let (mut device, connection) = device_with_snapshot(snapshot(backend, cards, 1));
    device.sync_questions(false);
    device.view.input.insert_str("main composer stays private");
    (device, connection)
}

fn replace(
    device: &mut Device,
    connection: ConnectionId,
    backend: Backend,
    cards: Vec<Approval>,
    sequence: u64,
) {
    let _ = device
        .client
        .receive(
            connection,
            Incoming::Notification(Box::new(Notification::SessionEvent(SessionEvent {
                session: device.session.clone(),
                seq: Sequence::new(sequence),
                event: Event::Reset {
                    snapshot: Box::new(snapshot(backend, cards, sequence)),
                },
            }))),
        )
        .expect("accepted current snapshot");
    device.sync_questions(false);
}

fn edit(device: &mut Device, index: u64, text: &str) {
    let draft = device
        .questions
        .drafts
        .get_mut(&id(index))
        .expect("private card editor");
    draft.editor.fields[0].paste(text);
    draft.confirm = true;
}

fn text(device: &Device, index: u64) -> String {
    device.questions.drafts[&id(index)].editor.fields[0]
        .editor
        .lines()
        .join("\n")
}

#[test]
fn native_reset_keeps_three_exact_private_drafts_and_confirmation_until_each_card_retires() {
    let cards: Vec<_> = (1..=3).map(card).collect();
    let (mut device, connection) = device(Backend::Claude, cards.clone());
    for index in 1..=3 {
        edit(&mut device, index, &format!("private {index}"));
    }
    replace(&mut device, connection, Backend::Claude, cards.clone(), 2);
    assert_eq!(device.questions.drafts.len(), 3);
    for index in 1..=3 {
        assert_eq!(text(&device, index), format!("private {index}"));
        assert!(device.questions.drafts[&id(index)].confirm);
    }
    replace(
        &mut device,
        connection,
        Backend::Claude,
        vec![cards[0].clone(), cards[2].clone()],
        3,
    );
    assert!(!device.questions.drafts.contains_key(&id(2)));
    assert_eq!(text(&device, 1), "private 1");
    assert_eq!(text(&device, 3), "private 3");
    let mut survivor = cards[2].clone();
    survivor.active = true;
    replace(&mut device, connection, Backend::Claude, vec![survivor], 4);
    assert_eq!(device.questions.drafts.len(), 1);
    assert_eq!(text(&device, 3), "private 3");
    assert!(device.questions.drafts[&id(3)].confirm);
    assert_eq!(
        device.view.input.lines().join("\n"),
        "main composer stays private"
    );
    assert_eq!(
        device.client.operations().iter().count(),
        0,
        "snapshot progress sends no answer or chat"
    );
}

#[test]
fn retiring_either_actual_device_card_keeps_the_other_answer_and_explicit_focus() {
    for backend in [Backend::Claude, Backend::Codex] {
        for cancelled in [1, 2] {
            let cards = vec![card(1), card(2)];
            let (mut device, connection) = device(backend, cards.clone());
            edit(&mut device, 1, "first unsent");
            edit(&mut device, 2, "second unsent");
            device.next_question();
            let remaining = if cancelled == 1 { 2 } else { 1 };
            // F4 deliberately disarms the newly focused card; arm this exact
            // surviving card after that key, before native retirement arrives.
            device
                .questions
                .drafts
                .get_mut(&id(remaining))
                .expect("survivor")
                .confirm = true;
            let mut survivor = cards[(remaining - 1) as usize].clone();
            survivor.active = true;
            replace(&mut device, connection, backend, vec![survivor], 2);
            assert_eq!(device.questions.drafts.len(), 1);
            assert!(!device.questions.drafts.contains_key(&id(cancelled)));
            assert_eq!(
                text(&device, remaining),
                if remaining == 1 {
                    "first unsent"
                } else {
                    "second unsent"
                }
            );
            assert!(device.questions.drafts[&id(remaining)].confirm);
            assert_eq!(device.questions.focus, Some(id(remaining)));
            assert_eq!(device.client.operations().iter().count(), 0);
        }
    }
}

#[test]
fn a_new_actual_device_cannot_inherit_an_old_composer_card_draft_or_confirmation() {
    for backend in [Backend::Claude, Backend::Codex, Backend::Grok] {
        let (mut old, _) = device(backend, vec![card(1)]);
        edit(&mut old, 1, "old unsent answer");
        let (fresh, _) = device_with_snapshot(snapshot(backend, Vec::new(), 1));
        assert!(fresh.questions.drafts.is_empty());
        assert!(fresh.view.input.lines().join("\n").is_empty());
        assert!(fresh.questions.focus.is_none());
        assert!(fresh
            .client
            .session(&fresh.session)
            .expect("fresh session")
            .approvals()
            .is_empty());
        assert_eq!(fresh.client.operations().iter().count(), 0);
        assert_eq!(text(&old, 1), "old unsent answer");
        assert!(old.questions.drafts[&id(1)].confirm);
    }
}

#[test]
fn a_blocking_child_gets_private_focus_without_changing_the_shared_nonblocking_root() {
    let mut root = card(1);
    root.blocking = false;
    root.display_id = Some("9".into());
    let mut child = card(2);
    child.display_id = Some("\"9\"".into());
    let (mut device, _) = device(Backend::Codex, vec![root.clone(), child.clone()]);
    assert_eq!(device.questions.focus, Some(child.id.clone()));
    assert!(device.questions.shown);
    assert_eq!(device.questions.drafts.len(), 2);
    edit(&mut device, 2, "private child answer");
    let shared = device
        .client
        .session(&device.session)
        .expect("accepted shared facts")
        .approvals();
    assert_eq!(shared, &[root.clone(), child.clone()]);
    assert_eq!(
        shared
            .iter()
            .find(|card| card.active)
            .expect("shared active root")
            .id,
        root.id
    );
    assert_eq!(text(&device, 2), "private child answer");
    assert_eq!(
        device.client.operations().iter().count(),
        0,
        "automatic private focus sends no answer"
    );
}

#[test]
fn a_waiting_browser_draft_survives_reset_until_its_exact_native_cancellation() {
    let mut waiting = card(1);
    waiting.input = Some(carlos_proto::session::ApprovalInput::Browser {
        url: "https://native-browser.test/original".into(),
    });
    let (mut device, connection) = device(Backend::Claude, vec![waiting.clone(), card(2)]);
    device
        .questions
        .drafts
        .get_mut(&id(1))
        .expect("browser consent draft")
        .confirm = true;
    replace(
        &mut device,
        connection,
        Backend::Claude,
        vec![waiting, card(2)],
        2,
    );
    assert!(device.questions.drafts[&id(1)].confirm);
    assert!(device.questions.drafts.contains_key(&id(2)));
    replace(&mut device, connection, Backend::Claude, vec![card(2)], 3);
    assert!(!device.questions.drafts.contains_key(&id(1)));
    assert!(device.questions.drafts.contains_key(&id(2)));
    assert_eq!(
        device.view.input.lines().join("\n"),
        "main composer stays private"
    );
    assert_eq!(device.client.operations().iter().count(), 0);
}

#[test]
fn escape_promotes_an_existing_blocking_card_without_answering_or_changing_shared_focus() {
    for blocking in [false, true] {
        let mut original = card(1);
        original.blocking = false;
        let (mut device, connection) = device(Backend::Codex, vec![original.clone()]);
        device.next_question();
        edit(&mut device, 1, "private nonblocking answer");
        let mut waiting = card(2);
        waiting.blocking = blocking;
        replace(
            &mut device,
            connection,
            Backend::Codex,
            vec![original, waiting],
            2,
        );
        assert_eq!(
            device.selected_approval().expect("explicit focus").id,
            id(1)
        );
        device
            .question_key(None, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .expect("escape from the nonblocking question");
        assert_eq!(
            device.selected_approval().map(|card| card.id.clone()),
            blocking.then(|| id(2))
        );
        assert_eq!(text(&device, 1), "private nonblocking answer");
        assert_eq!(device.questions.drafts.len(), 2);
        let accepted = device
            .client
            .session(&device.session)
            .expect("accepted shared cards")
            .approvals();
        assert!(accepted[0].active);
        assert!(!accepted[1].active);
        assert_eq!(
            device.view.input.lines().join("\n"),
            "main composer stays private"
        );
        assert_eq!(device.client.operations().iter().count(), 0);
    }
}
