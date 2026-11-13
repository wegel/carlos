//! Native items and checked history can update a session without an app fixture.

use serde_json::{json, Value};

use super::item_changes::ItemChanges;
use super::item_history::replace_history_from_thread;
use super::notification_items::handle_item_notification;
use super::state::SessionState;
use crate::session::models::Role;

#[test]
fn native_tool_completion_keeps_streamed_output_without_any_view() {
    for backend in ["codex", "claude", "grok"] {
        let mut session = SessionState::new("root".into(), Default::default(), false);
        if backend == "codex" {
            session.enable_codex();
        }
        if backend == "claude" {
            session.enable_claude();
        }
        session.replace_codex_item_turn(Some("turn".into()));
        let mut changes = ItemChanges::default();
        for (method, params) in [
            (
                "item/started",
                json!({"item":{"id":"cmd","type":"commandExecution","command":"cargo test"}}),
            ),
            (
                "item/commandExecution/outputDelta",
                json!({"itemId":"cmd","delta":"tests passed\n"}),
            ),
            (
                "item/completed",
                json!({"item":{"id":"cmd","type":"commandExecution","command":"cargo test","status":"completed"}}),
            ),
        ] {
            assert!(handle_item_notification(
                &mut session,
                &mut changes,
                method,
                params.as_object().expect("fixed item")
            ));
        }
        let key = session.backend_entry_key("cmd");
        let index = session.item_message_index(&key).expect("native row");
        assert_eq!(
            session.messages()[index].text,
            "$ cargo test\ntests passed\n"
        );
        assert_eq!(session.messages()[index].role, Role::ToolOutput);
        assert!(session.is_item_completed(&key));
        assert!(session.messages()[index].tool_output.is_some());
        assert!(!changes.transcript.is_empty());
    }
}

#[test]
fn checked_history_retains_native_ids_and_rejects_another_thread_atomically() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    let first = history(json!([item("a", "old a"), item("b", "old b")]));
    replace_history_from_thread(&mut session, &first).expect("first history");
    let a = session.messages()[0].id;
    let b = session.messages()[1].id;
    let next = history(json!([
        item("x", "new"),
        item("a", "old a"),
        item("b", "corrected")
    ]));
    replace_history_from_thread(&mut session, &next).expect("same thread");
    assert_eq!(session.messages()[1].id, a);
    assert_eq!(session.messages()[2].id, b);
    assert_eq!(session.messages()[2].text, "corrected");
    let mut foreign = next;
    foreign["id"] = json!("other");
    assert!(replace_history_from_thread(&mut session, &foreign).is_err());
    assert_eq!(session.messages().len(), 3);
    assert_eq!(session.messages()[2].id, b);
}

fn item(id: &str, text: &str) -> Value {
    json!({"type":"agentMessage","id":id,"text":text})
}

fn history(items: Value) -> Value {
    json!({"id":"root","turns":[{"id":"turn","status":"completed","items":items}]})
}
