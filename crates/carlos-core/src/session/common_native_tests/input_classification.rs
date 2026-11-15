//! Keep each prompt's native UUID and author class independent of a device's mode.

use serde_json::json;

use super::{feed, session};
use crate::claude_backend::FixtureNormalizer;
use crate::session::models::{MessageKind, Role};

#[test]
fn captured_claude_uuid_kinds_survive_later_mode_changes_and_do_not_change_text_or_editability() {
    let mut app = session("root");
    app.set_claude_enabled_for_test(true);
    for (id, kind) in [
        ("normal", "prompt"),
        ("steer", "steer"),
        ("queued", "prompt"),
    ] {
        let index = app.append_message(Role::User, "identical 日本語");
        app.record_prompt_history("identical 日本語", Some(app.messages()[index].id));
        app.receive_claude_submission(&json!({"result":{"turn":{"id":id},"inputKind":kind}}));
    }
    assert_eq!(
        app.messages()
            .iter()
            .map(|message| message.kind)
            .collect::<Vec<_>>(),
        vec![
            MessageKind::Plain,
            MessageKind::UserSteer,
            MessageKind::Plain
        ]
    );
    assert!(app
        .messages()
        .iter()
        .all(|message| message.text == "identical 日本語"));
    let queued = app.input_history().entry_id(2).expect("queued prompt");
    let steer = app.input_history().entry_id(1).expect("steer prompt");
    assert_eq!(app.input_history().turn_id(queued), Some("queued"));
    assert_eq!(app.input_history().turn_id(steer), Some("steer"));
    assert!(
        app.input_history().native_request(1, steer).is_none(),
        "Claude never uses the Codex steer-refusal ledger"
    );
}

#[test]
fn loaded_classification_never_promotes_unknown_or_automatic_records_to_human() {
    let mut app = session("root");
    app.set_claude_enabled_for_test(true);
    let records: Vec<_> = [("human",Some("prompt")),("human",Some("steer")),("human",None),("automatic",Some("steer")),("unknown",Some("steer"))].into_iter().enumerate().map(|(index,(source,kind))|
        json!({"source":source,"inputKind":kind,"message":{"type":"user","uuid":format!("id-{index}"),"session_id":"root","message":{"role":"user","content":"same"}}})).collect();
    let input = json!({"method":"claude/controller","params":{"requestMethod":"sessions.read","frame":{"type":"reply","ok":true,"result":{"messages":records,"nextOffset":null}}}});
    app.set_claude_generation_for_test(1);
    let mut backend = FixtureNormalizer::default();
    for line in backend.lines(&input, "root", 1) {
        feed(&mut app, &line);
    }
    assert_eq!(app.input_history().history_len(), 3);
    assert_eq!(app.messages()[0].kind, MessageKind::Plain);
    assert_eq!(app.messages()[1].kind, MessageKind::UserSteer);
    assert_eq!(app.messages()[2].kind, MessageKind::Plain);
    assert!(app.messages()[3..]
        .iter()
        .all(|message| message.role == Role::System && message.kind != MessageKind::UserSteer));
}
