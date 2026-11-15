//! Enumerate browser identity, consent order, callback cancellation, and clear lifetimes.

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use serde_json::{json, Value};

use super::approval_control_test_support::Peer;
use super::approval_state::ApprovalChoice;
use super::control_work::ControlFailure;
use super::state::SessionState;
use crate::backend::BackendKind;

fn browser(app: &mut SessionState, id: &str) {
    app.receive_claude_callback(&json!({"callbackId":id,"kind":"elicitation","payload":{
        "mode":"url","serverName":"server","elicitationId":id,"url":"https://example.com/ask"
    }}));
}

fn completed(app: &mut SessionState, id: &str) {
    let _ = app.receive_claude_browser_completion(
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
        let mut app = SessionState::new("root".into(), Default::default(), false);
        app.register_claude_browser("callback", &payload);
        assert_eq!(app.claude().browser_request_count(), 0);
        for message in [
            Value::Null,
            json!({"mcp_server_name":"s"}),
            json!({"mcp_server_name":"s","elicitation_id":"id"}),
        ] {
            let _ = app.receive_claude_browser_completion(&message);
        }
        assert!(app.messages().is_empty());
    }
}

#[test]
fn browser_completion_requires_one_notice_in_either_consent_order() {
    for early in [false, true] {
        let mut app = SessionState::new("root".into(), Default::default(), false);
        browser(&mut app, "id");
        if early {
            completed(&mut app, "id");
            assert!(app.export_approvals()[0].details.iter().any(|detail| detail ==
                "The server reported browser completion. Your consent is still required; decline or cancel remains available."
            ));
        }
        respond(
            &mut app,
            &Peer::new(BackendKind::Claude),
            ApprovalChoice::Accept,
        );
        assert!(app.approval().pending().is_none());
        for _ in 0..2 {
            completed(&mut app, "id");
        }
        assert_eq!(app.claude().browser_request_count(), 0);
        assert_eq!(
            app.messages()
                .iter()
                .filter(|row| row.text == "Browser interaction completed for server")
                .count(),
            1
        );
        let consent = app
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
        let mut app = SessionState::new("root".into(), Default::default(), false);
        browser(&mut app, "id");
        let peer = Peer::new(BackendKind::Claude);
        // The old mock failed before writing. A definite typed validation rejection
        // preserves that safe-to-correct case; uncertain writes stay claimed elsewhere.
        assert!(matches!(
            write(
                &mut app,
                &peer,
                ApprovalAnswer::Input { value: Value::Null }
            ),
            Err(ControlFailure::Invalid(_))
        ));
        assert!(peer.calls.lock().expect("no native write").is_empty());
        assert_eq!(app.claude().browser_request_count(), 1);
        assert!(app.messages().is_empty());
        completed(&mut app, "id");
        respond(&mut app, &peer, choice);
        assert_eq!(app.claude().browser_request_count(), 0);
        completed(&mut app, "id");
        assert_eq!(app.messages().len(), 1);
    }
}

#[test]
fn clear_keeps_consented_and_unanswered_browser_requests_until_their_native_end() {
    let mut app = SessionState::new("root".into(), Default::default(), false);
    browser(&mut app, "consented");
    respond(
        &mut app,
        &Peer::new(BackendKind::Claude),
        ApprovalChoice::Accept,
    );
    browser(&mut app, "waiting");
    app.receive_claude_reset(
        &json!({"new_conversation_id":"clear"}),
        &mut Default::default(),
    );
    assert_eq!(app.claude().browser_request_count(), 2);
    assert_eq!(
        app.approval()
            .pending()
            .expect("waiting browser")
            .facts()
            .request_id,
        "waiting"
    );
    completed(&mut app, "consented");
    completed(&mut app, "waiting");
    assert_eq!(app.claude().browser_request_count(), 1);
    app.cancel_claude_callback(&json!("other"));
    assert_eq!(app.claude().browser_request_count(), 1);
    app.cancel_claude_callback(&json!("waiting"));
    assert_eq!(app.claude().browser_request_count(), 0);
    completed(&mut app, "waiting");
    assert_eq!(app.messages().len(), 2);
}

#[test]
fn accepted_unregistered_browser_payload_still_uses_the_existing_notice_defaults() {
    let mut app = SessionState::new("root".into(), Default::default(), false);
    browser(&mut app, "id");
    let mut pending = app.approval().pending().expect("callback").to_draft();
    pending.requested_permissions =
        Some(json!({"mode":"url","serverName":"other\nserver","elicitationId":"missing"}));
    app.finish_claude_browser_answer(&pending, ApprovalChoice::Accept);
    assert_eq!(app.messages()[0].text, "Browser consent sent to other\\nserver. Link: not supplied. Complete the interaction in your browser; the server has not reported completion.");
    assert_eq!(app.claude().browser_request_count(), 1);
}

fn write(
    core: &mut SessionState,
    peer: &Peer,
    answer: ApprovalAnswer,
) -> Result<(), ControlFailure> {
    let card = core
        .approval()
        .pending()
        .expect("live browser callback")
        .entry_id();
    let work = core
        .prepare_engine_answer(peer, card, answer)
        .expect("claim exact callback");
    core.finish_engine_control(peer, work.execute(&peer.commands))
}

fn respond(core: &mut SessionState, peer: &Peer, choice: ApprovalChoice) {
    let decision = match choice {
        ApprovalChoice::Accept => Decision::Once,
        ApprovalChoice::Decline => Decision::Deny,
        ApprovalChoice::Cancel => Decision::Cancel,
        _ => panic!("unexpected consent choice"),
    };
    let before = peer.calls.lock().expect("native answers").len();
    write(core, peer, ApprovalAnswer::Decision { decision }).expect("accepted native answer");
    assert_eq!(
        peer.calls.lock().expect("one exact answer").len(),
        before + 1
    );
}
