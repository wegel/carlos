//! Verify that clients receive neutral fields rather than backend callback payloads.

use carlos_proto::session::ApprovalInput;
use serde_json::json;

use super::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};
use super::state::SessionState;

#[test]
fn codex_and_claude_questions_use_the_same_field_contract() {
    for kind in [
        ApprovalRequestKind::CodexQuestion,
        ApprovalRequestKind::ClaudeNative,
    ] {
        let mut core = SessionState::new("native-thread".into(), Default::default(), false);
        let question = json!({"id":"answer-key","header":"Choice","question":"Which?",
            "options":[{"label":"Yes","description":"Proceed"}],"multiSelect":true});
        let payload = if kind == ApprovalRequestKind::CodexQuestion {
            json!({"threadId":"secret-thread","questions":[question]})
        } else {
            json!({"toolName":"AskUserQuestion","toolUseId":"secret-tool",
                "input":{"questions":[question]}})
        };
        let _ = core.set_pending_approval(ApprovalRequestDraft {
            request_id: json!("secret-request"),
            method: "native".into(),
            kind,
            title: "Choose".into(),
            detail_lines: Vec::new(),
            requested_permissions: Some(payload),
            can_accept_for_session: false,
            can_decline: true,
            can_cancel: true,
        });
        let snapshot = core.publish().snapshot(100);
        let ApprovalInput::Fields { fields } = snapshot.approvals[0].input.as_ref().expect("input")
        else {
            panic!("neutral fields");
        };
        assert_eq!(fields[0].title, "Choice");
        assert_eq!(fields[0].description, "Which?");
        assert_eq!(fields[0].options[0].value, json!("Yes"));
        assert_eq!(
            fields[0].multiple,
            kind == ApprovalRequestKind::ClaudeNative
        );
        let wire = serde_json::to_string(&snapshot).expect("snapshot");
        for secret in [
            "secret-thread",
            "secret-tool",
            "secret-request",
            "native-thread",
        ] {
            assert!(!wire.contains(secret));
        }
    }
}

#[test]
fn unsupported_question_input_is_explicit_and_does_not_offer_empty_accept() {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    let _ = core.set_pending_approval(ApprovalRequestDraft {
        request_id: json!(1),
        method: "native".into(),
        kind: ApprovalRequestKind::CodexQuestion,
        title: "Question".into(),
        detail_lines: Vec::new(),
        requested_permissions: Some(json!({"questions":[]})),
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    });
    let snapshot = core.publish().snapshot(100);
    assert!(matches!(
        snapshot.approvals[0].input,
        Some(ApprovalInput::Unsupported { .. })
    ));
    assert_eq!(
        snapshot.approvals[0].choices,
        vec![carlos_proto::session::Decision::Deny]
    );
}

#[test]
fn approval_focus_moves_keep_the_client_card_order() {
    use super::approval_state::ApprovalQueue;
    let mut core = SessionState::new("native".into(), Default::default(), false);
    let make_card = |id| ApprovalRequestDraft {
        request_id: json!(id),
        method: "native".into(),
        kind: ApprovalRequestKind::CommandExecution,
        title: "Question".into(),
        detail_lines: Vec::new(),
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    };
    let _ = core.set_pending_approval(make_card(1));
    core.enqueue_approval(make_card(2), ApprovalQueue::Claude);
    let third = core.enqueue_approval(make_card(3), ApprovalQueue::Claude);
    let mut client = core.publish().snapshot(100);
    core.focus_approval(third, ApprovalQueue::Claude);
    let batch = core.publish();
    for record in batch.records() {
        let event = batch.to_wire(record).expect("event");
        if let carlos_proto::events::Event::ApprovalRequested { approval, before } = event.event {
            client.approvals.retain(|old| old.id != approval.id);
            let index = before
                .and_then(|id| client.approvals.iter().position(|old| old.id == id))
                .unwrap_or(client.approvals.len());
            client.approvals.insert(index, approval);
        }
    }
    assert_eq!(client.approvals, batch.snapshot(100).approvals);
    assert_eq!(client.approvals[0].id, third.to_wire());
}

#[test]
fn an_interrupted_turn_finishes_unclosed_streams_and_keeps_its_text() {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    core.record_turn_started("turn");
    let key = core.backend_entry_key("answer");
    core.upsert_agent_delta(&key, "partial 🌍");
    assert!(!core.publish().snapshot(100).entries[0].done);
    assert!(core.record_turn_completed("turn", super::models::TurnStatus::Interrupted));
    let batch = core.publish();
    let snapshot = batch.snapshot(100);
    assert!(snapshot.entries[0].done);
    assert_eq!(snapshot.entries[0].text, "partial 🌍");
    assert!(batch
        .records()
        .iter()
        .any(|record| matches!(record.change, super::events::Change::Facts(_))));
    assert_eq!(snapshot.info.phase, carlos_proto::session::Phase::Idle);
}

#[test]
fn real_backend_handlers_feed_the_same_json_client_for_all_backends() {
    for backend in [
        carlos_proto::session::Backend::Codex,
        carlos_proto::session::Backend::Claude,
        carlos_proto::session::Backend::Grok,
    ] {
        let mut core = SessionState::new("root".into(), Default::default(), false);
        core.set_client_identity(backend, "local".into());
        if backend == carlos_proto::session::Backend::Codex {
            core.enable_codex();
        }
        if backend == carlos_proto::session::Backend::Claude {
            core.enable_claude();
        }
        let mut client = core.publish().snapshot(100);
        for line in super::session_headless_tests::common_lines() {
            core.handle_backend_line(
                &line.to_string(),
                &super::backend_test_support::context(&core),
            );
            super::event_tests::synchronize(&mut core, &mut client);
        }
        assert_eq!(client.entries[0].text, "hello world");
        assert!(client.entries[0].done);
        assert_eq!(client.info.context.expect("context").used, 2500);
        assert_eq!(client.info.phase, carlos_proto::session::Phase::Idle);
        assert_eq!(client.info.backend, backend);
    }
}

#[test]
fn browser_consent_offers_once_and_retains_the_backend_default_no_fact() {
    for kind in [
        ApprovalRequestKind::CodexForm,
        ApprovalRequestKind::ClaudeQuestion,
    ] {
        let mut core = SessionState::new("native".into(), Default::default(), false);
        let _ = core.set_pending_approval(ApprovalRequestDraft {
            request_id: json!("private-route"),
            method: "native".into(),
            kind,
            title: "Open browser".into(),
            detail_lines: Vec::new(),
            requested_permissions: Some(
                json!({"mode":"url","url":"https://example.com/consent","defaultToNo":true}),
            ),
            can_accept_for_session: false,
            can_decline: true,
            can_cancel: true,
        });
        let snapshot = core.publish().snapshot(100);
        let card = &snapshot.approvals[0];
        assert!(card.default_to_no);
        assert!(card
            .choices
            .contains(&carlos_proto::session::Decision::Once));
        assert!(matches!(card.input, Some(ApprovalInput::Browser { .. })));
        assert!(!serde_json::to_string(card)
            .expect("card")
            .contains("private-route"));
    }
}

#[test]
fn permission_cards_never_offer_an_unsupported_cancel_reply() {
    let mut core = SessionState::new("native".into(), Default::default(), false);
    let _ = core.set_pending_approval(ApprovalRequestDraft {
        request_id: json!(1),
        method: "native".into(),
        kind: ApprovalRequestKind::Permissions,
        title: "Permissions".into(),
        detail_lines: Vec::new(),
        requested_permissions: Some(json!({"network":true})),
        can_accept_for_session: true,
        can_decline: true,
        can_cancel: true,
    });
    let snapshot = core.publish().snapshot(100);
    assert_eq!(
        snapshot.approvals[0].choices,
        vec![
            carlos_proto::session::Decision::Once,
            carlos_proto::session::Decision::Session,
            carlos_proto::session::Decision::Deny
        ]
    );
}
