//! Enumerate native identity bounds, replay windows and form-worker ordering.

use serde_json::{json, Value};

use super::super::codex_question_support::{app, question, receive};

use super::split_peer::ValidationPeer;
use crate::app::codex_callbacks::{checked, reply_hint, retire};

use crate::app::codex_reply::respond;
use crate::app::native_questions::draft;
use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::approval_state::ApprovalChoice;

#[test]
fn callback_ids_keep_json_types_and_both_original_byte_bounds() {
    for (id, accepted) in [
        (json!(""), true),
        (json!(9), true),
        (json!("9"), true),
        (json!(-3), true),
        (json!(1.5), true),
        (json!(u64::MAX), true),
        (json!("a".repeat(4094)), true),
        (json!("a".repeat(4095)), false),
        (json!("\n".repeat(2047)), true),
        (json!("\n".repeat(2048)), false),
        (Value::Null, false),
        (json!(true), false),
        (json!([]), false),
        (json!({}), false),
    ] {
        let mut app = app();
        let action = receive(&mut app, question(id.clone(), "child", true));
        if accepted {
            assert!(action.is_none(), "{id}");
            assert_eq!(
                app.session
                    .approval()
                    .pending()
                    .expect("accepted")
                    .facts()
                    .request_id,
                id
            );
        } else {
            assert!(
                matches!(
                    action,
                    Some(ServerRequestAction::ReplyError { code: -32600, .. })
                ),
                "{id}"
            );
            assert!(app.session.approval().pending().is_none());
        }
    }
}

#[test]
fn duplicate_and_retired_checks_precede_unsupported_methods_and_keep_2048_ids() {
    let mut app = app();
    receive(&mut app, question(json!("live"), "child", true));
    let id = app.session.approval().pending().expect("card").entry_id();
    assert!(receive(&mut app, json!({"id":"live","method":"future/request"})).is_none());
    assert_eq!(
        app.session
            .approval()
            .pending()
            .expect("same card")
            .entry_id(),
        id
    );
    retire(&mut app, &json!("live"));
    for n in 0..2047 {
        retire(&mut app, &json!(n));
    }
    retire(&mut app, &json!("live")); // A replay does not refresh its place in the window.
    assert!(receive(&mut app, json!({"id":"live","method":"future/request"})).is_none());
    retire(&mut app, &json!(2047));
    assert!(matches!(
        receive(&mut app, json!({"id":"live","method":"future/request"})),
        Some(ServerRequestAction::ReplyError { code: -32601, .. })
    ));
    assert!(receive(&mut app, json!({"id":0,"method":"future/request"})).is_none());
}

fn form() -> Value {
    json!({"id":"form","method":"mcpServer/elicitation/request","params":{
        "threadId":"child","serverName":"s","mode":"form",
        "requestedSchema":{"type":"object","properties":{}}}})
}

#[test]
fn schema_preflight_keeps_busy_notice_and_worker_failure_keeps_the_candidate() {
    for preflight_ok in [false, true] {
        let mut app = app();
        let peer = ValidationPeer::default();
        receive(&mut app, form());
        respond(&peer, &mut app, ApprovalChoice::Accept);
        assert_eq!(peer.checks.lock().expect("checks").len(), 1);
        assert!(checked(
            &mut app,
            &json!({"requestId":"form","revision":0,"ok":preflight_ok})
        )
        .is_none());
        assert_eq!(
            draft(&app).expect("draft").notice.as_deref(),
            Some("Checking the form schema…")
        );
        assert_eq!(
            draft(&app).expect("draft").unsupported.as_deref(),
            if preflight_ok {
                None
            } else {
                Some("Unsupported form schema")
            }
        );
        reply_hint(&mut app, &json!("form"), "worker failed");
        let action = checked(
            &mut app,
            &json!({"requestId":"form","revision":1,"ok":true}),
        );
        assert!(
            matches!(action, Some(ServerRequestAction::ReplyNative { result, .. })
            if result == json!({"action":"accept","content":{}}))
        );
        assert!(peer.peer.replies.borrow().is_empty());
    }
}

#[test]
fn clear_and_native_reset_keep_the_unresolved_process_callback_and_editor() {
    use crate::app::approval_lifetime_support::{app, callback, Client};

    use crate::app::native_questions::draft_mut;
    use carlos_core::backend::BackendKind;

    for native_reset in [false, true] {
        let client = Client::new(BackendKind::Claude);
        let mut app = app(BackendKind::Claude);
        callback(
            &client,
            &mut app,
            "process",
            "elicitation",
            json!({
            "requestedSchema":{"type":"object","properties":{"value":{"type":"string"}}}}),
        );
        let card = app
            .session
            .approval()
            .pending()
            .expect("process card")
            .entry_id();
        draft_mut(&mut app).expect("editor").fields[0].paste("unsent answer");
        crate::app::turn_submit::submit_turn_text(&client, &mut app, "/clear".into());
        assert!(client.answers().is_empty());
        let calls = client.calls.lock().expect("calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0][0], "turn/start");
        assert_eq!(calls[0][1]["input"][0]["text"], "/clear");
        drop(calls);
        if native_reset {
            crate::app::claude_reset::receive(&mut app, &json!({"new_conversation_id":"reset"}));
            assert!(app.session.messages().is_empty());
        }
        assert_eq!(
            app.session
                .approval()
                .pending()
                .expect("same card")
                .entry_id(),
            card
        );
        assert_eq!(
            draft(&app).expect("same editor").fields[0].editor.lines(),
            &["unsent answer"]
        );
    }
}
