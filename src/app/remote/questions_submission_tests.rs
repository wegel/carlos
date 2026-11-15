//! Keep failed native answers private and mask only the currently visible secret editor.

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_proto::commands::{ApprovalAnswer, Command, RequestId};
use carlos_proto::ids::Sequence;
use carlos_proto::session::{Approval, Backend, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::AnswerTarget;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::remote::Device;

fn device(secret: bool) -> (Device, ConnectionId) {
    let card: Approval = serde_json::from_value(json!({
        "id":format!("{}-0000000000000001", "a".repeat(32)), "kind":"question",
        "title":"Native question", "details":[], "choices":["once","deny","cancel"],
        "input":{"kind":"fields","fields":[{"key":"q","title":"Field","description":"Question?",
        "secret":secret,"multiple":false,"other":true,"notes_only":false,"required":true,"options":[],"schema":{"type":"string"}}]},
        "active":true,"blocking":false,"submitted":false,"default_to_no":false,
    })).expect("typed native field");
    let mut metadata = info("a".repeat(32).parse().expect("session"), false);
    metadata.backend = Backend::Codex;
    let (mut device, connection) = device_with_snapshot(Snapshot {
        info: metadata,
        entries: Vec::new(),
        approvals: vec![card],
        seq: Sequence::new(1),
        has_older: false,
        goal: None,
        queue: None,
        tasks: None,
        settings: None,
    });
    device.sync_questions(false);
    device.next_question();
    device.view.input.insert_str("main stays");
    let id = device.selected_approval().expect("local focus").id.clone();
    device
        .questions
        .drafts
        .get_mut(&id)
        .expect("private field")
        .editor
        .fields[0]
        .editor
        .insert_str("answer stays");
    (device, connection)
}

fn submitted(device: &mut Device, connection: ConnectionId) -> (RequestId, ClientOperation) {
    let session = device.session.clone();
    let card = device
        .selected_approval()
        .expect("original target")
        .id
        .clone();
    let mut original = None;
    let outgoing = device
        .client
        .submit(connection, |operation| {
            original = Some(operation.clone());
            Command::Approve {
                session,
                operation,
                approval: card.clone(),
                answer: ApprovalAnswer::Input {
                    value: json!({"fields":[{"key":"q","values":[],"text":"answer stays"}]}),
                },
            }
        })
        .expect("explicit original answer");
    let operation = original.expect("originating private operation");
    device.questions.operations.insert(
        operation.clone(),
        AnswerTarget {
            fingerprint: device.questions.drafts[&card].fingerprint,
            card,
            finished: false,
            interrupt: false,
        },
    );
    (outgoing.id(), operation)
}

#[test]
fn a_rejected_or_uncertain_native_answer_keeps_the_original_question_and_main_drafts() {
    for uncertain in [false, true] {
        let (mut device, connection) = device(false);
        let card = device
            .selected_approval()
            .expect("original card")
            .id
            .clone();
        let (id, operation) = submitted(&mut device, connection);
        let state = if uncertain {
            OperationState::Uncertain {
                message: "native write failed".into(),
                recovery: None,
            }
        } else {
            OperationState::Rejected {
                message: "native answer refused".into(),
                recovery: None,
            }
        };
        let effects = device
            .client
            .receive(
                connection,
                Incoming::Success {
                    id,
                    result: serde_json::to_value(OperationResult {
                        operation: operation.clone(),
                        session: Some(device.session.clone()),
                        rewound: false,
                        state,
                    })
                    .expect("private native outcome"),
                },
            )
            .expect("current original reply");
        assert_eq!(effects.operations, vec![operation.clone()]);
        assert!(
            effects.outgoing.is_empty(),
            "no automatic answer or chat replay"
        );
        assert!(device.apply_question_operation(&operation));
        let draft = &device.questions.drafts[&card];
        assert_eq!(draft.editor.fields[0].editor.lines(), &["answer stays"]);
        assert!(draft.editor.error.is_some());
        assert_eq!(device.selected_approval().expect("same live card").id, card);
        assert_eq!(device.view.input.lines().join("\n"), "main stays");
    }
}

#[test]
fn secret_review_masks_only_the_visible_native_question_and_not_the_hidden_main_composer() {
    let (mut device, _) = device(true);
    let card = device
        .selected_approval()
        .expect("focused secret card")
        .id
        .clone();
    device
        .questions
        .drafts
        .get_mut(&card)
        .expect("original editor")
        .editor
        .review = true;
    assert!(device.question_input_is_secret());
    device
        .question_key(None, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        .expect("explicitly hide nonblocking review");
    assert!(!device.question_input_is_secret());
    assert_eq!(device.view.input.lines().join("\n"), "main stays");
    assert_eq!(
        device.questions.drafts[&card].editor.fields[0]
            .editor
            .lines(),
        &["answer stays"]
    );
    assert_eq!(device.client.operations().iter().count(), 0);
}
