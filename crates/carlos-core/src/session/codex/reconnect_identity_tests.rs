//! Keep repeated native IDs distinct across turns, reconnects and display-only parts.

use serde_json::json;

use super::connection_test_support::snapshot;
use super::reconnect_install::install;
use crate::session::backend_test_support::context;
use crate::session::item_history::replace_history_from_thread;
use crate::session::models::Role;
use crate::session::state::SessionState;

fn session() -> SessionState {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_codex_enabled_for_test(true);
    session.enable_native_history();
    session
}

fn handle_server_message_line(session: &mut SessionState, line: &str) {
    session.handle_backend_line(line, &context(session));
    session.publish();
}

fn message(app: &mut SessionState, turn: &str, kind: &str, text: &str) {
    handle_server_message_line(app,
        &json!({"method":"item/completed","params":{"threadId":"root","turnId":turn,
        "item":{"id":if kind=="userMessage" {"same-user"} else {"same-answer"},"type":kind,"text":text,"content":[{"type":"text","text":text}]}}}).to_string());
}

#[test]
fn repeated_native_item_and_user_ids_stay_distinct_in_live_history_and_reconnect() {
    let mut app = session();
    app.enable_native_history();
    for turn in ["one", "two"] {
        message(&mut app, turn, "userMessage", turn);
        message(&mut app, turn, "agentMessage", &format!("answer-{turn}"));
    }
    assert_eq!(app.messages().len(), 4);
    let history = json!({"id":"root","status":{"type":"idle"},"turns":[
        {"id":"one","status":"completed","items":[{"id":"same-user","type":"userMessage","content":[{"type":"text","text":"identical"}]},{"id":"same-answer","type":"agentMessage","text":"answer-one"}]},
        {"id":"two","status":"completed","items":[{"id":"same-user","type":"userMessage","content":[{"type":"text","text":"identical"}]},{"id":"same-answer","type":"agentMessage","text":"answer-two"}]}]});
    replace_history_from_thread(&mut app, &history).expect("same-thread history");
    assert_eq!(app.messages().len(), 4);
    assert_eq!(app.messages()[1].text, "answer-one");
    assert_eq!(app.messages()[3].text, "answer-two");
    app.replace_codex_item_turn(Some("outside".into()));
    handle_server_message_line(&mut app,&json!({"method":"item/agentMessage/delta","params":{"threadId":"root","turnId":"one","itemId":"same-answer","delta":"LATE_OLD"}}).to_string());
    assert_eq!(app.codex_item_turn().as_deref(), Some("outside"));
    assert_eq!(app.messages()[3].text, "answer-two");
    app.replace_codex_item_turn(None);
    let index = app.input_history().history_len() - 1;
    let entry = app
        .input_history()
        .entry_id(index)
        .expect("last native prompt");
    let selected = app
        .input_history()
        .native_request(index, entry)
        .expect("native user anchor");
    assert_eq!(selected.item_id, "same-user");
    assert_eq!(app.input_history().turn_id(entry), Some("two"));
    app.close_codex();
    app.set_codex_generation_for_test(2);
    app.set_codex_rebuilding_for_test(true);
    let mut current = snapshot();
    current.thread = history;
    install(&mut app, current, None).expect("reconnect native snapshot");
    assert_eq!(
        app.messages()
            .iter()
            .filter(|row| row.role == Role::Assistant)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        ["answer-one", "answer-two"]
    );
}

#[test]
fn native_suffix_and_unicode_ids_cannot_collide_with_display_parts() {
    let mut app = session();
    let item = json!({"type":"toolOutput","id":"x","files":[
        {"path":"a.rs","kind":{"type":"update"},"diff":"@@ -1 +1 @@\n-old-a\n+new-a\n"},
        {"path":"b.rs","kind":{"type":"update"},"diff":"@@ -1 +1 @@\n-old-b\n+new-b\n"}]});
    replace_history_from_thread(
        &mut app,
        &json!({"id":"root","turns":[{"id":"日本語:","status":"completed","items":[
        item,{"id":"x#part-1","type":"agentMessage","text":"NATIVE_SUFFIX"}]}]}),
    )
    .expect("same-thread history");
    assert_eq!(app.messages().len(), 3);
    assert!(app.messages()[0].text.contains("new-a"));
    assert!(app.messages()[1].text.contains("new-b"));
    assert_eq!(app.messages()[2].text, "NATIVE_SUFFIX");
    app.replace_codex_item_turn(Some("a:b".into()));
    let first = app.backend_entry_key("日本語:c");
    app.replace_codex_item_turn(Some("a".into()));
    assert_ne!(first, app.backend_entry_key("b:日本語:c"));
    assert_eq!(app.codex_item_part(), 0);
}
