//! Keeps Codex steer labels and exact rewind anchors consistent across live and loaded views.

use serde_json::{json, Value};

use super::identity_support::reset;
use super::{feed as handle_notification_line, session};
use crate::backend::BackendKind;
use crate::session::command_work::{BackendCall, Finish};
use crate::session::item_history::load_history_from_start_or_resume;
use crate::session::models::{MessageKind, Role};
use crate::session::prompt_commands::PromptContext;
use crate::session::state::SessionState;

fn user(app: &mut SessionState, method: &str, turn: &str, id: &str, text: &str) {
    handle_notification_line(app,&json!({"method":method,"params":{"threadId":app.thread_id(),
        "turnId":turn,"item":{"type":"userMessage","id":id,"content":[{"type":"text","text":text}]}}}).to_string());
}
fn enabled() -> SessionState {
    let mut app = session("thread");
    app.enable_native_history();
    app
}
fn snapshot() -> String {
    json!({"result":{"thread":{"id":"thread","turns":[{"id":"shared","status":"completed","items":[
        {"type":"userMessage","id":"first","content":[{"type":"text","text":"same 日本語"}]},
        {"type":"userMessage","id":"steer","content":[{"type":"text","text":"same 日本語"}]}]}]}}})
    .to_string()
}

#[test]
fn live_started_and_completed_events_count_each_exact_user_item_once() {
    let mut app = enabled();
    for method in ["item/started", "item/completed"] {
        user(&mut app, method, "shared", "first", "same 日本語");
    }
    for method in ["item/started", "item/completed", "item/completed"] {
        user(&mut app, method, "shared", "steer", "same 日本語");
    }
    assert_eq!(app.messages().len(), 2);
    assert_eq!(app.messages()[0].kind, MessageKind::Plain);
    assert_eq!(app.messages()[1].kind, MessageKind::UserSteer);
    assert!(app
        .messages()
        .iter()
        .all(|message| message.role == Role::User));
    let entry = app.input_history().entry_id(1).expect("steer prompt");
    let selected = app
        .input_history()
        .native_request(1, entry)
        .expect("exact anchor");
    assert_eq!(selected.item_id, "steer");
    assert_eq!(selected.position, 1);
    let first = app.input_history().entry_id(0).expect("first prompt");
    assert_eq!(
        app.input_history()
            .native_request(0, first)
            .expect("first anchor")
            .item_id,
        "first"
    );
}

#[test]
fn busy_first_message_is_normal_and_hidden_earlier_inputs_still_count() {
    let mut app = enabled();
    app.record_turn_started("new-turn");
    user(&mut app, "item/completed", "new-turn", "normal", "normal");
    assert_eq!(app.messages()[0].kind, MessageKind::Plain);
    user(&mut app, "item/completed", "other-turn", "empty", "");
    user(
        &mut app,
        "item/completed",
        "other-turn",
        "after-empty",
        "visible",
    );
    assert_eq!(
        app.messages().last().expect("visible").kind,
        MessageKind::UserSteer
    );
}

#[test]
fn replacement_and_repeated_fork_resets_keep_the_ledger_enabled_and_anchors_exact() {
    let mut app = enabled();
    for _ in 0..3 {
        reset(&mut app, "thread");
        load_history_from_start_or_resume(&mut app, &snapshot()).expect("loaded native users");
        assert!(app.input_history().native_enabled());
        assert_eq!(app.messages()[1].kind, MessageKind::UserSteer);
        let entry = app.input_history().entry_id(1).expect("steer prompt");
        assert_eq!(
            app.input_history()
                .native_request(1, entry)
                .expect("selected")
                .item_id,
            "steer"
        );
    }
}

#[test]
fn other_backends_keep_plain_user_messages_and_existing_grouping() {
    let mut app = session("thread");
    for id in ["first", "second"] {
        user(&mut app, "item/completed", "shared", id, "same");
    }
    assert!(app
        .messages()
        .iter()
        .all(|message| message.kind == MessageKind::Plain));
    assert!(!app.input_history().native_enabled());
}

#[test]
fn edited_draft_does_not_replace_the_original_canonical_text_in_the_request() {
    let mut app = enabled();
    load_history_from_start_or_resume(&mut app, &snapshot()).expect("history");
    let prompt = app.input_history().prompt_id(1).expect("steer prompt");
    let work = app
        .prepare_rewind_text(
            BackendKind::Codex,
            prompt,
            "changed draft".into(),
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
            false,
        )
        .expect("prepare edit")
        .work
        .expect("fork work");
    let BackendCall::Fork { request, .. } = &work.call else {
        panic!("fork call")
    };
    let selected = request.before_user_item.as_ref().expect("native anchor");
    assert_eq!(selected.text, "same 日本語");
    assert_eq!(selected.item_id, "steer");
    let expected: Value = json!({"type":"text","text":selected.text});
    assert!(!expected.to_string().contains("steer"));
    let Finish::Rewind { submission, .. } = work.finish else {
        panic!("edit completion")
    };
    assert_eq!(submission.text, "changed draft");
}
