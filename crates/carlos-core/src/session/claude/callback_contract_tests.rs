//! Preserve native callback facts, unknown withdrawals and exact once-only browser completion.

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::{ApprovalInput, Decision};
use serde_json::json;

use crate::backend::BackendKind;
use crate::session::approval_control_test_support::Peer;
use crate::session::approval_state::{ApprovalChoice, ApprovalQueue};
use crate::session::control_work::ControlFailure;
use crate::session::state::SessionState;

#[test]
fn native_plan_without_text_remains_declineable() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    core.receive_claude_callback(&json!({"callbackId":"plan","kind":"permission",
        "payload":{"toolName":"ExitPlanMode","input":{"planFilePath":"/tmp/missing.md"}}}));
    let pending = core.approval().pending().expect("visible native request");
    assert_eq!(
        pending
            .facts()
            .response_for_choice(ApprovalChoice::Decline)
            .expect("decline remains available")["behavior"],
        "deny"
    );
}

#[test]
fn incomplete_native_server_metadata_uses_unknown_labels_without_panicking() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    core.receive_claude_callback(
        &json!({ "callbackId": "tool", "kind": "permission", "payload": {
        "toolName": "mcp__fixture__read", "input": {}, "mcpServer": {}
    } }),
    );
    assert!(core
        .approval()
        .pending()
        .expect("visible native request")
        .facts()
        .detail_lines
        .iter()
        .any(|line| line == "MCP server: unknown (source: unknown)"));
}

#[test]
fn unknown_callback_withdrawal_preserves_every_other_request() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    for id in ["first", "second"] {
        core.receive_claude_callback(
            &json!({ "callbackId": id, "kind": "elicitation", "payload": {
            "requestedSchema": { "type": "object", "properties": { "name": { "type": "string" } } }
        } }),
        );
    }
    core.cancel_claude_callback(&json!("unknown"));
    assert_eq!(
        core.approval()
            .pending()
            .expect("active request")
            .facts()
            .request_id,
        "first"
    );
    assert_eq!(core.approval().queued(ApprovalQueue::Claude).len(), 1);
}

#[test]
fn browser_completion_matches_server_and_id_once_without_consuming_other_servers() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    for server in ["one", "two"] {
        core.receive_claude_callback(
            &json!({ "callbackId": server, "kind": "elicitation", "payload": {
            "mode": "url", "serverName": server, "elicitationId": "same-id", "url": "https://example.com/ask"
        } }),
        );
    }
    core.receive_claude_browser_completion(
        &json!({ "mcp_server_name": "unknown", "elicitation_id": "same-id" }),
    );
    assert_eq!(core.claude().browser_request_count(), 2);
    for _ in 0..2 {
        core.receive_claude_browser_completion(
            &json!({ "mcp_server_name": "one", "elicitation_id": "same-id" }),
        );
    }
    assert_eq!(core.messages().len(), 1);
    assert_eq!(core.claude().browser_request_count(), 2);
    assert_eq!(
        core.approval()
            .pending()
            .expect("consent still pending")
            .facts()
            .request_id,
        "one"
    );
    let pending = core
        .approval()
        .pending()
        .expect("explicit consent request")
        .to_draft();
    core.finish_claude_browser_answer(&pending, ApprovalChoice::Accept);
    assert!(core
        .messages()
        .last()
        .expect("accurate completion notice")
        .text
        .contains("already reported completion"));
    assert_eq!(core.claude().browser_request_count(), 1);
    assert!(core.has_claude_browser_request_for_test("two", "same-id"));
    core.cancel_claude_callback(&json!("two"));
    assert!(core.claude().browser_request_count() == 0);
}

#[test]
fn unsupported_plan_rejects_forged_accepts_without_a_native_write() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    core.receive_claude_callback(&json!({"callbackId":"plan","kind":"permission",
        "payload":{"toolName":"ExitPlanMode","input":{"planFilePath":"/tmp/missing.md"}}}));
    let id = core.approval().pending().expect("plan card").entry_id();
    let card = core.export_approvals().remove(0);
    assert!(
        matches!(card.input, Some(ApprovalInput::Unsupported { ref reason }) if reason.contains("no plan text"))
    );
    let peer = Peer::new(BackendKind::Claude);
    for decision in [Decision::Once, Decision::Session] {
        assert!(!card.choices.contains(&decision));
        let work = core
            .prepare_engine_answer(&peer, id, ApprovalAnswer::Decision { decision })
            .expect("claim exact card");
        assert!(matches!(
            core.finish_engine_control(&peer, work.execute(&peer.commands)),
            Err(ControlFailure::Invalid(_))
        ));
        assert!(peer.calls.lock().expect("native answers").is_empty());
        assert!(!core
            .approval()
            .pending()
            .expect("same live card")
            .submitted());
    }
    let work = core
        .prepare_engine_answer(
            &peer,
            id,
            ApprovalAnswer::Decision {
                decision: Decision::Deny,
            },
        )
        .expect("decline remains offered");
    core.finish_engine_control(&peer, work.execute(&peer.commands))
        .expect("native decline");
    assert_eq!(
        peer.calls.lock().expect("native answers").as_slice(),
        &[(
            json!("plan"),
            json!({"behavior":"deny","message":"User declined"})
        )]
    );
}

#[test]
fn escaped_choice_labels_return_the_original_native_value() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    core.receive_claude_callback(&json!({"callbackId":"choice","kind":"permission",
        "payload":{"toolName":"AskUserQuestion","input":{"questions":[{"question":"Which?",
        "header":"Choice","multiSelect":false,"options":[{"label":"Line\n\u{1b}[2J","description":"Choose explicitly"},
        {"label":"Other choice","description":"Alternative"}]}]}}}));
    let card = core.export_approvals().remove(0);
    let Some(ApprovalInput::Fields { fields }) = &card.input else {
        panic!("typed native choice")
    };
    let answer = ApprovalAnswer::Input {
        value: json!({"fields":[{
        "key":fields[0].key,"values":[fields[0].options[0].value],"text":null}]}),
    };
    let facts = core.approval().pending().expect("original request").facts();
    let (_, body) = crate::session::control_answers::convert(facts, &card, &answer)
        .expect("original native envelope");
    assert_eq!(body["updatedInput"]["answers"]["Which?"], "Line\n\u{1b}[2J");
}
