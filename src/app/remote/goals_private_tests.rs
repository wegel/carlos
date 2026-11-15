//! Keep visible typed cards ahead of a retained goal editor after the real F4 handoff.

use carlos_client::codec::Incoming;
use carlos_client::ConnectionId;
use carlos_proto::commands::{Command, RequestId};
use carlos_proto::goals::{Goal, GoalChange, GoalFacts, GoalKnowledge, GoalStatus};
use carlos_proto::ids::Sequence;
use carlos_proto::session::{Approval, ApprovalInput, Backend, Snapshot};
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use serde_json::{json, Value};

use crate::app::codex_goal_state::GoalDraft;
use crate::app::remote::widget_fixture::{device_with_snapshot, info};
use crate::app::remote::Device;
use crate::app::view_models::TerminalSize;

fn device(approvals: Vec<Approval>, goal: Option<GoalFacts>) -> (Device, ConnectionId) {
    let mut metadata = info("a".repeat(32).parse().expect("session"), false);
    metadata.backend = Backend::Codex;
    device_with_snapshot(Snapshot {
        info: metadata,
        entries: Vec::new(),
        approvals,
        seq: Sequence::new(1),
        has_older: false,
        goal,
        queue: None,
        tasks: None,
        settings: None,
    })
}

fn hidden_goal(input: ApprovalInput) -> Device {
    let approval: Approval = serde_json::from_value(json!({
        "id":format!("{}-0000000000000001", "a".repeat(32)), "kind":"question",
        "title":"Native request", "details":[], "choices":["once","deny","cancel"],
        "input":input, "active":true, "blocking":true, "submitted":false, "default_to_no":false,
    }))
    .expect("typed native request");
    let (mut device, _) = device(vec![approval], None);
    device.sync_questions(false);
    device.questions.size = Some(TerminalSize {
        width: 100,
        height: 30,
    });
    device.view.input.insert_str("keep main draft");
    let mut editor = GoalDraft::new(&Value::Null);
    editor.objective.insert_str("keep goal draft");
    device.view.codex_goals.editor = Some(editor);
    device.view.codex_goals.editing = true;
    device.view.codex_goals.show = true;
    device
        .event(
            None,
            Event::Key(KeyEvent::new(KeyCode::F(4), KeyModifiers::NONE)),
        )
        .expect("F4 closes goal panel and focuses the native card");
    assert!(!device.view.codex_goals.show);
    assert!(device.view.codex_goals.editing);
    device
}

fn unchanged(device: &Device) {
    assert_eq!(
        device
            .view
            .codex_goals
            .editor
            .as_ref()
            .expect("retained goal")
            .objective
            .lines()
            .join("\n"),
        "keep goal draft"
    );
    assert_eq!(device.view.input.lines().join("\n"), "keep main draft");
    assert_eq!(device.client.operations().iter().count(), 0);
}

#[test]
fn a_visible_form_owns_paste_and_wheel_while_the_goal_editor_is_hidden() {
    let mut device = hidden_goal(ApprovalInput::Form {
        schema: json!({"type":"object","properties":{"value":{"type":"string"}}}),
    });
    device
        .event(None, Event::Paste("FORM_ONLY".into()))
        .expect("paste into native form");
    let id = device
        .selected_approval()
        .expect("visible original form")
        .id
        .clone();
    let draft = &device.questions.drafts[&id];
    assert_eq!(draft.editor.fields[0].editor.lines(), &["FORM_ONLY"]);
    draft.editor.viewport.max_scroll.set(20);
    device
        .event(
            None,
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 1,
                row: 1,
                modifiers: KeyModifiers::NONE,
            }),
        )
        .expect("wheel belongs to visible native card");
    assert_eq!(device.questions.drafts[&id].editor.viewport.scroll.get(), 3);
    assert_eq!(device.view.codex_goals.scroll, 0);
    unchanged(&device);
}

#[test]
fn an_unsupported_typed_card_swallows_paste_without_a_field_or_hidden_goal_edit() {
    let mut device = hidden_goal(ApprovalInput::Unsupported {
        reason: "Unsupported native form".into(),
    });
    device
        .event(None, Event::Paste("MUST_NOT_EDIT".into()))
        .expect("unsupported card consumes paste");
    let id = &device
        .selected_approval()
        .expect("same unsupported card")
        .id;
    assert!(device.questions.drafts[id].editor.fields.is_empty());
    unchanged(&device);
}

fn facts(knowledge: GoalKnowledge, goal: Option<Goal>) -> GoalFacts {
    GoalFacts {
        revision: "b".repeat(64).parse().expect("goal revision"),
        knowledge,
        goal,
        reading: false,
        pending: false,
        direct_input: Some(true),
        read_error: None,
        notice: String::new(),
    }
}

fn saved() -> Goal {
    Goal {
        objective: "last good objective".into(),
        status: "active".into(),
        token_budget: None,
        tokens_used: Sequence::new(7),
        time_used_seconds: Sequence::new(9),
        created_at: 1,
        updated_at: 2,
    }
}

fn retain_editor(device: &mut Device) {
    device.view.codex_goals.editor = Some(GoalDraft::new(&json!({
        "objective":"retained private objective", "status":"active",
    })));
    device.view.codex_goals.editing = true;
    device.view.input.insert_str("private main composer");
}

fn retained_editor(device: &Device) {
    assert_eq!(
        device
            .view
            .codex_goals
            .editor
            .as_ref()
            .expect("private editor")
            .objective
            .lines()
            .join("\n"),
        "retained private objective"
    );
    assert!(device.view.codex_goals.editing);
    assert_eq!(
        device.view.input.lines().join("\n"),
        "private main composer"
    );
}

#[test]
fn unknown_goal_facts_keep_the_editor_and_a_cleared_goal_never_prepares_activation() {
    for unknown in [true, false] {
        let knowledge = if unknown {
            GoalKnowledge::Unknown
        } else {
            GoalKnowledge::Known
        };
        let (mut device, _) = device(Vec::new(), Some(facts(knowledge, unknown.then(saved))));
        retain_editor(&mut device);
        let error = device
            .save_goal(None)
            .expect_err("no write while unknown or disconnected")
            .to_string();
        assert!(
            error.contains(if unknown {
                "Read the current"
            } else {
                "Disconnected"
            }),
            "{error}"
        );
        assert!(
            device.goals.confirm.is_none(),
            "a cleared goal cannot default to activation"
        );
        assert_eq!(device.client.operations().iter().count(), 0);
        assert_eq!(
            device.goal_value()["objective"],
            if unknown {
                json!("last good objective")
            } else {
                Value::Null
            }
        );
        retained_editor(&device);
    }
}

fn confirmed_activation(
    device: &mut Device,
    connection: ConnectionId,
) -> (RequestId, ClientOperation) {
    let session = device.session.clone();
    let expected = device.goal_facts().expect("accepted goal").revision.clone();
    let mut original = None;
    let outgoing = device
        .client
        .submit(connection, |operation| {
            original = Some(operation.clone());
            Command::GoalSet {
                session,
                operation,
                expected,
                change: GoalChange {
                    objective: Some("retained private objective".into()),
                    token_budget: None,
                    status: GoalStatus::Active,
                },
            }
        })
        .expect("original confirmed goal intent");
    let operation = original.expect("private operation");
    device.goals.operation = Some(operation.clone());
    device.goals.clear_editor = true;
    (outgoing.id(), operation)
}

#[test]
fn rejected_or_uncertain_activation_keeps_the_same_devices_goal_and_main_drafts() {
    for uncertain in [false, true] {
        let (mut device, connection) =
            device(Vec::new(), Some(facts(GoalKnowledge::Known, Some(saved()))));
        retain_editor(&mut device);
        let (id, operation) = confirmed_activation(&mut device, connection);
        let state = if uncertain {
            OperationState::Uncertain {
                message: "malformed native activation reply".into(),
                recovery: None,
            }
        } else {
            OperationState::Rejected {
                message: "native activation refused".into(),
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
                    .expect("exact private result"),
                },
            )
            .expect("own result accepted");
        assert_eq!(effects.operations, vec![operation.clone()]);
        assert!(effects.outgoing.is_empty(), "no automatic retry");
        assert!(device.apply_goal_operation(&operation));
        assert!(device.view.codex_goals.error.is_some());
        retained_editor(&device);
    }
}
