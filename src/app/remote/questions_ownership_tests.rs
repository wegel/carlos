//! Keep consent, hints and edited answers private to actual daemon devices.

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_proto::events::{Event, Notification, SessionEvent};
use carlos_proto::ids::{EntryId, Sequence};
use carlos_proto::session::{Approval, ApprovalInput, Backend, Phase, Snapshot};
use crossterm::event::{Event as TerminalEvent, KeyCode, KeyEvent, KeyModifiers};
use serde_json::{json, Value};

use super::Device;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::view_models::TerminalSize;

fn card(index: u64, input: Option<ApprovalInput>) -> Approval {
    let id: EntryId = format!("{}-{index:016x}", "a".repeat(32))
        .parse()
        .expect("card");
    serde_json::from_value(json!({"id":id,"kind":"tool","title":"Original callback",
        "details":["native details"],"choices":["once","deny","cancel"],"input":input,
        "active":true,"blocking":true,"submitted":false,"default_to_no":true}))
    .expect("typed card")
}

fn device(backend: Backend, card: Approval) -> (Device, ConnectionId) {
    let mut info = info("a".repeat(32).parse().expect("session"), false);
    info.backend = backend;
    info.phase = Phase::Approval;
    let (mut device, connection) = device_with_snapshot(Snapshot {
        info,
        entries: Vec::new(),
        approvals: vec![card],
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        settings: None,
        tasks: None,
    });
    device.questions.size = Some(TerminalSize {
        width: 100,
        height: 30,
    });
    device.sync_questions(false);
    (device, connection)
}

fn facts(device: &Device) -> Value {
    let replica = device
        .client
        .session(&device.session)
        .expect("accepted session");
    json!({"info":replica.info(),"entries":replica.entries().collect::<Vec<_>>(),
        "approvals":replica.approvals(),"goal":replica.goal(),"queue":replica.queue(),
        "tasks":replica.tasks(),"settings":replica.settings(),"sequence":replica.sequence(),"older":replica.has_older(),
        "newer":replica.has_newer(),"stale":replica.is_stale()})
}

fn first_key(device: &mut Device, input: Option<&ApprovalInput>) {
    let (code, modifiers) = if matches!(
        input,
        Some(ApprovalInput::Fields { .. } | ApprovalInput::Form { .. })
    ) {
        (KeyCode::Char('s'), KeyModifiers::CONTROL)
    } else {
        (KeyCode::Char('y'), KeyModifiers::NONE)
    };
    let result = device.question_key(None, KeyEvent::new(code, modifiers));
    if matches!(input, Some(ApprovalInput::Unsupported { .. })) {
        assert!(result
            .expect_err("unsupported request cannot accept")
            .to_string()
            .contains("unsupported"));
    } else {
        assert!(result.expect("first consent is local"));
    }
}

fn inputs() -> Vec<Option<ApprovalInput>> {
    let fields = serde_json::from_value(json!({"kind":"fields","fields":[{
        "key":"exact-field","title":"Field","description":"Prompt","secret":false,
        "multiple":false,"other":true,"notes_only":false,"required":false,
        "schema":{"type":"string"},"options":[]}]}))
    .expect("fields");
    vec![
        None,
        Some(fields),
        Some(ApprovalInput::Form {
            schema: json!({"type":"object","properties":{}}),
        }),
        Some(ApprovalInput::Browser {
            url: "https://example.com/consent".into(),
        }),
        Some(ApprovalInput::Unsupported {
            reason: "unsupported original request".into(),
        }),
    ]
}

#[test]
fn first_consent_and_local_hints_leave_every_accepted_shared_fact_unchanged() {
    for backend in [Backend::Codex, Backend::Claude, Backend::Grok] {
        for input in inputs() {
            let (mut device, _) = device(backend, card(1, input.clone()));
            let before = facts(&device);
            first_key(&mut device, input.as_ref());
            assert_eq!(facts(&device), before, "{backend:?} {input:?}");
            assert_eq!(device.client.operations().iter().count(), 0);
            device.view.status_hint = Some("local editor error".into());
            device
                .event(
                    None,
                    TerminalEvent::Key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)),
                )
                .expect("local selection key");
            assert_eq!(facts(&device), before);
            assert_eq!(device.client.operations().iter().count(), 0);
        }
    }
}

fn replace(device: &mut Device, connection: ConnectionId, seq: u64, card: Approval) {
    let event = SessionEvent {
        session: device.session.clone(),
        seq: Sequence::new(seq),
        event: Event::Reset {
            snapshot: Box::new(Snapshot {
                info: device
                    .client
                    .session(&device.session)
                    .expect("session")
                    .info()
                    .clone(),
                entries: Vec::new(),
                approvals: vec![card],
                seq: Sequence::new(seq),
                has_older: false,
                goal: None,
                queue: None,
                settings: None,
                tasks: None,
            }),
        },
    };
    let _ = device
        .client
        .receive(
            connection,
            Incoming::Notification(Box::new(Notification::SessionEvent(event))),
        )
        .expect("accepted replacement facts");
    device.sync_questions(false);
}

#[test]
fn new_opaque_card_cannot_inherit_old_consent_while_duplicate_metadata_keeps_it() {
    let original = card(1, None);
    let (mut device, connection) = device(Backend::Claude, original.clone());
    first_key(&mut device, None);
    assert!(device.questions.drafts[&original.id].confirm);
    replace(&mut device, connection, 2, original.clone());
    assert!(device.questions.drafts[&original.id].confirm);
    let replacement = card(2, None);
    replace(&mut device, connection, 3, replacement.clone());
    assert!(!device.questions.drafts.contains_key(&original.id));
    assert!(!device.questions.drafts[&replacement.id].confirm);
    first_key(&mut device, None);
    assert!(device.questions.drafts[&replacement.id].confirm);
    assert_eq!(
        device.client.operations().iter().count(),
        0,
        "new card needs its own first consent key"
    );
}
