//! Enumerate browser identity, consent order, callback cancellation, and clear lifetimes.

use serde_json::{json, Value};

use super::claude_browser::complete;
use super::claude_callbacks::{cancel_callback, receive_callback};

use super::state::AppState;
use super::turn_approval::respond_to_pending_approval;
use crate::app::claude_process_test_support as support;
use crate::session::approval_state::ApprovalChoice;
use support::Peer;

fn browser(app: &mut AppState, id: &str) {
    receive_callback(
        app,
        &json!({"callbackId":id,"kind":"elicitation","payload":{
            "mode":"url","serverName":"server","elicitationId":id,"url":"https://example.com/ask"
        }}),
    );
}

fn completed(app: &mut AppState, id: &str) {
    complete(
        app,
        &json!({"mcp_server_name":"server","elicitation_id":id}),
    );
}

#[test]
fn invalid_browser_identity_never_registers_or_reports_a_completion() {
    for payload in [
        Value::Null,
        json!({"mode":"form"}),
        json!({"mode":"url","serverName":"s"}),
        json!({"mode":"url","serverName":7,"elicitationId":"id"}),
    ] {
        let mut app = AppState::new("root".into());
        app.session.register_claude_browser("callback", &payload);
        assert_eq!(app.session.claude().browser_request_count(), 0);
        for message in [
            Value::Null,
            json!({"mcp_server_name":"s"}),
            json!({"mcp_server_name":"s","elicitation_id":"id"}),
        ] {
            complete(&mut app, &message);
        }
        assert!(app.session.messages().is_empty());
    }
}

#[test]
fn browser_completion_requires_one_notice_in_either_consent_order() {
    for early in [false, true] {
        let mut app = AppState::new("root".into());
        browser(&mut app, "id");
        if early {
            completed(&mut app, "id");
            assert_eq!(app.view.claude_answers.drafts["id"].notice.as_deref(), Some(
                "The server reported browser completion. Your consent is still required; decline or cancel remains available."
            ));
        }
        respond_to_pending_approval(&Peer::default(), &mut app, ApprovalChoice::Accept);
        assert!(app.view.claude_answers.drafts.is_empty());
        for _ in 0..2 {
            completed(&mut app, "id");
        }
        assert_eq!(app.session.claude().browser_request_count(), 0);
        assert_eq!(
            app.session
                .messages()
                .iter()
                .filter(|row| row.text == "Browser interaction completed for server")
                .count(),
            1
        );
        let consent = app
            .session
            .messages()
            .iter()
            .find(|row| row.text.starts_with("Browser consent sent"))
            .expect("consent notice");
        assert_eq!(
            consent.text,
            if early {
                "Browser consent sent to server. Link: https://example.com/ask. The server already reported completion."
            } else {
                "Browser consent sent to server. Link: https://example.com/ask. Complete the interaction in your browser; the server has not reported completion."
            }
        );
    }
}

#[test]
fn failed_answer_and_decline_or_cancel_keep_their_distinct_browser_lifetimes() {
    for choice in [ApprovalChoice::Decline, ApprovalChoice::Cancel] {
        let mut app = AppState::new("root".into());
        browser(&mut app, "id");
        let peer = Peer::default();
        peer.fail.set(true);
        respond_to_pending_approval(&peer, &mut app, ApprovalChoice::Accept);
        assert_eq!(app.session.claude().browser_request_count(), 1);
        assert!(app.session.messages().is_empty());
        completed(&mut app, "id");
        peer.fail.set(false);
        respond_to_pending_approval(&peer, &mut app, choice);
        assert_eq!(app.session.claude().browser_request_count(), 0);
        completed(&mut app, "id");
        assert_eq!(app.session.messages().len(), 1);
    }
}

#[test]
fn clear_keeps_consented_and_unanswered_browser_requests_until_their_native_end() {
    let mut app = AppState::new("root".into());
    browser(&mut app, "consented");
    respond_to_pending_approval(&Peer::default(), &mut app, ApprovalChoice::Accept);
    browser(&mut app, "waiting");
    super::claude_reset::receive(&mut app, &json!({"new_conversation_id":"clear"}));
    assert_eq!(app.session.claude().browser_request_count(), 2);
    assert!(app.view.claude_answers.drafts.contains_key("waiting"));
    completed(&mut app, "consented");
    completed(&mut app, "waiting");
    assert_eq!(app.session.claude().browser_request_count(), 1);
    cancel_callback(&mut app, &json!("other"));
    assert_eq!(app.session.claude().browser_request_count(), 1);
    cancel_callback(&mut app, &json!("waiting"));
    assert_eq!(app.session.claude().browser_request_count(), 0);
    completed(&mut app, "waiting");
    assert_eq!(app.session.messages().len(), 2);
}

#[test]
fn accepted_unregistered_browser_payload_still_uses_the_existing_notice_defaults() {
    let mut app = AppState::new("root".into());
    browser(&mut app, "id");
    let mut pending = app
        .session
        .approval()
        .pending()
        .expect("callback")
        .to_draft();
    pending.requested_permissions =
        Some(json!({"mode":"url","serverName":"other\nserver","elicitationId":"missing"}));
    super::claude_browser::finish_answer(&mut app, &pending, ApprovalChoice::Accept);
    assert_eq!(app.session.messages()[0].text, "Browser consent sent to other\\nserver. Link: not supplied. Complete the interaction in your browser; the server has not reported completion.");
    assert_eq!(app.session.claude().browser_request_count(), 1);
}
