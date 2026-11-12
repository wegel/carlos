//! Preserve independent editors and late-validation fences during the parser split.

use serde_json::json;

use super::super::codex_question_support::{app, field_text, question, receive, Peer};
use crate::app::claude_callbacks::{cancel_callback, receive_callback};
use crate::app::codex_callbacks::{checked, edited, next, retire};
use crate::app::codex_reply::send;
use crate::app::notifications::ServerRequestAction;
use crate::app::state::AppState;
use crate::session::codex::callbacks::CallbackKey;

#[test]
fn cancelling_either_typed_callback_keeps_the_other_editor_and_focus() {
    for cancelled in [json!(9), json!("9")] {
        let mut app = app();
        for (id, text) in [(json!(9), "numeric"), (json!("9"), "string")] {
            receive(&mut app, question(id.clone(), "child", true));
            app.view
                .codex_callbacks
                .drafts
                .get_mut(&CallbackKey::new(&id).expect("typed id"))
                .expect("callback")
                .fields[0]
                .paste(text);
        }
        next(&mut app);
        retire(&mut app, &cancelled);
        let (remaining, text) = if cancelled.is_number() {
            (json!("9"), "string")
        } else {
            (json!(9), "numeric")
        };
        assert_eq!(field_text(&app, remaining.clone()), text);
        assert!(app.session.codex_callbacks().get(&cancelled).is_none());
        assert_eq!(app.session.codex_callbacks().len(), 1);
        assert_eq!(app.view.codex_callbacks.drafts.len(), 1);
        assert!(!app
            .view
            .codex_callbacks
            .drafts
            .contains_key(&CallbackKey::new(&cancelled).expect("typed id")));
        assert_eq!(
            app.session
                .approval()
                .pending()
                .expect("survivor")
                .facts()
                .request_id,
            remaining
        );
    }
}

#[test]
fn cancelling_either_claude_question_keeps_the_other_editor() {
    for cancelled in ["first", "second"] {
        let mut app = AppState::new("root".into());
        for id in ["first", "second"] {
            receive_callback(
                &mut app,
                &json!({"callbackId":id,"kind":"elicitation","payload":{
                "requestedSchema":{"properties":{"x":{"type":"string"}}}}}),
            );
            app.view
                .claude_answers
                .drafts
                .get_mut(id)
                .expect("draft")
                .fields[0]
                .paste(id);
        }
        cancel_callback(&mut app, &json!(cancelled));
        let remaining = if cancelled == "first" {
            "second"
        } else {
            "first"
        };
        assert_eq!(app.view.claude_answers.drafts.len(), 1);
        assert_eq!(
            app.view.claude_answers.drafts[remaining].fields[0]
                .editor
                .lines(),
            &[remaining]
        );
        assert_eq!(
            app.session
                .approval()
                .pending()
                .expect("survivor")
                .facts()
                .request_id,
            remaining
        );
    }
}

#[test]
fn late_form_checks_cannot_send_after_candidate_revision_or_lifetime_changes() {
    for change in ["candidate", "revision", "retired", "wrong-id"] {
        let mut app = app();
        receive(
            &mut app,
            json!({"id":"form","method":"mcpServer/elicitation/request","params":{
            "threadId":"child","serverName":"s","mode":"form","requestedSchema":{"properties":{}}}}),
        );
        let card = app.session.approval().pending().expect("card").entry_id();
        app.session.begin_approval_answer(card);
        let candidate =
            (change != "candidate").then(|| json!({"action":"accept","content":{"x":"old"}}));
        app.session
            .seed_codex_candidate_for_test(&json!("form"), 2, candidate, Some(card), true);
        match change {
            "revision" => edited(&mut app),
            "retired" => retire(&mut app, &json!("form")),
            _ => {}
        }
        let id = if change == "wrong-id" {
            "absent"
        } else {
            "form"
        };
        let action = checked(&mut app, &json!({"requestId":id,"revision":2,"ok":true}));
        let peer = Peer::default();
        if let Some(ServerRequestAction::ReplyNative { card_id, result }) = action {
            send(&peer, &mut app, card_id, result);
        }
        assert!(peer.replies.borrow().is_empty(), "{change}");
    }
}

#[test]
fn callbacks_arriving_after_root_completion_still_receive_independent_editors() {
    let mut app = app();
    receive(
        &mut app,
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"t"}}}),
    );
    receive(
        &mut app,
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"t","status":"completed"}}}),
    );
    receive(&mut app, question(json!("late"), "child", true));
    app.view
        .codex_callbacks
        .drafts
        .get_mut(&CallbackKey::new(&json!("late")).expect("typed id"))
        .expect("late callback")
        .fields[0]
        .paste("answer");
    assert_eq!(field_text(&app, json!("late")), "answer");
    assert!(app.session.active_turn_id().is_none());
}

#[test]
fn native_reset_keeps_the_unresolved_process_question_and_its_draft() {
    let mut app = AppState::new("root".into());
    receive_callback(
        &mut app,
        &json!({"callbackId":"process","kind":"elicitation","payload":{
        "requestedSchema":{"properties":{"x":{"type":"string"}}}}}),
    );
    app.view
        .claude_answers
        .drafts
        .get_mut("process")
        .expect("draft")
        .fields[0]
        .paste("unsent");
    crate::app::claude_reset::receive(&mut app, &json!({"new_conversation_id":"reset"}));
    assert_eq!(
        app.view.claude_answers.drafts["process"].fields[0]
            .editor
            .lines(),
        &["unsent"]
    );
    assert_eq!(
        app.session
            .approval()
            .pending()
            .expect("process callback")
            .facts()
            .request_id,
        "process"
    );
    assert!(app.session.messages().is_empty());
}
