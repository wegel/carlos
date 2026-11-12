//! Check native callback identity, missing plans, and display-safe question labels.

use serde_json::json;

use super::claude_browser::{complete, finish_answer};
use super::claude_callbacks::{cancel_callback, receive_callback};
use super::claude_question_state::{safe_label, NativeDraft};
use super::state::{AppState, ApprovalChoice};
use crate::session::approval_state::ApprovalQueue;

#[test]
fn native_plan_without_text_cannot_be_approved_but_can_be_declined() {
    let payload =
        json!({ "toolName": "ExitPlanMode", "input": { "planFilePath": "/tmp/missing.md" } });
    let draft = NativeDraft::from_payload("permission", &payload);
    assert!(draft
        .answer(&payload)
        .expect_err("missing plan must fail")
        .contains("no plan text"));
    let mut app = AppState::new("session".to_string());
    receive_callback(
        &mut app,
        &json!({ "callbackId": "plan", "kind": "permission", "payload": payload }),
    );
    let pending = app
        .session
        .approval()
        .pending()
        .expect("visible native request");
    assert_eq!(
        pending
            .facts()
            .response_for_choice(ApprovalChoice::Decline)
            .expect("decline remains available")["behavior"],
        "deny"
    );
}

#[test]
fn labels_escape_controls_while_native_answers_retain_the_original_value() {
    let payload = json!({ "toolName": "AskUserQuestion", "input": { "questions": [{
        "question": "Which?", "header": "Choice", "multiSelect": false,
        "options": [{ "label": "Line\n\u{1b}[2J", "description": "Choose explicitly" }, { "label": "Other choice", "description": "Alternative" }]
    }] } });
    let mut draft = NativeDraft::from_payload("permission", &payload);
    draft.fields[0].select();
    assert_eq!(
        safe_label(draft.fields[0].spec.options()[0].label()),
        "Line\\n\\u{1b}[2J"
    );
    assert_eq!(
        draft.answer(&payload).expect("native answer")["updatedInput"]["answers"]["Which?"],
        "Line\n\u{1b}[2J"
    );
}

#[test]
fn incomplete_native_server_metadata_uses_unknown_labels_without_panicking() {
    let mut app = AppState::new("session".to_string());
    receive_callback(
        &mut app,
        &json!({ "callbackId": "tool", "kind": "permission", "payload": {
        "toolName": "mcp__fixture__read", "input": {}, "mcpServer": {}
    } }),
    );
    assert!(app
        .session
        .approval()
        .pending()
        .expect("visible native request")
        .facts()
        .detail_lines
        .iter()
        .any(|line| line == "MCP server: unknown (source: unknown)"));
}

#[test]
fn unknown_callback_withdrawal_preserves_every_other_request_and_draft() {
    let mut app = AppState::new("session".to_string());
    for id in ["first", "second"] {
        receive_callback(
            &mut app,
            &json!({ "callbackId": id, "kind": "elicitation", "payload": {
            "requestedSchema": { "type": "object", "properties": { "name": { "type": "string" } } }
        } }),
        );
        app.claude
            .answers
            .drafts
            .get_mut(id)
            .expect("independent draft")
            .fields[0]
            .paste(id);
    }
    cancel_callback(&mut app, &json!("unknown"));
    assert_eq!(
        app.session
            .approval()
            .pending()
            .expect("active request")
            .facts()
            .request_id,
        "first"
    );
    assert_eq!(
        app.session.approval().queued(ApprovalQueue::Claude).len(),
        1
    );
    assert_eq!(
        app.claude.answers.drafts["second"].fields[0].editor.lines(),
        &["second"]
    );
}

#[test]
fn browser_completion_matches_server_and_id_once_without_consuming_other_servers() {
    let mut app = AppState::new("session".to_string());
    for server in ["one", "two"] {
        receive_callback(
            &mut app,
            &json!({ "callbackId": server, "kind": "elicitation", "payload": {
            "mode": "url", "serverName": server, "elicitationId": "same-id", "url": "https://example.com/ask"
        } }),
        );
    }
    complete(
        &mut app,
        &json!({ "mcp_server_name": "unknown", "elicitation_id": "same-id" }),
    );
    assert_eq!(app.session.claude().browser_request_count(), 2);
    for _ in 0..2 {
        complete(
            &mut app,
            &json!({ "mcp_server_name": "one", "elicitation_id": "same-id" }),
        );
    }
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.claude().browser_request_count(), 2);
    assert_eq!(
        app.session
            .approval()
            .pending()
            .expect("consent still pending")
            .facts()
            .request_id,
        "one"
    );
    let pending = app
        .session
        .approval()
        .pending()
        .expect("explicit consent request")
        .to_draft();
    finish_answer(&mut app, &pending, ApprovalChoice::Accept);
    assert!(app
        .session
        .messages()
        .last()
        .expect("accurate completion notice")
        .text
        .contains("already reported completion"));
    assert_eq!(app.session.claude().browser_request_count(), 1);
    assert!(app
        .session
        .has_claude_browser_request_for_test("two", "same-id"));
    cancel_callback(&mut app, &json!("two"));
    assert!(app.session.claude().browser_request_count() == 0);
}
