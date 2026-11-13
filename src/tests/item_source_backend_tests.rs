//! Keep native Codex and Grok rows bound when Claude has no evidence about them.

use serde_json::{json, Value};

use super::entry_identity_tests::app;
use crate::app::notifications::handle_server_message_line;
use carlos_core::backend::BackendKind;
use carlos_core::session::entry_key::NativeEntryKey;
use carlos_core::session::models::{MessageKind, Role};

fn check_stream(events: &[Value], text: &str) {
    for backend in [BackendKind::Codex, BackendKind::Grok] {
        let mut app = app(backend);
        app.session.set_summary_enabled_for_test(false);
        // Codex wins when both flags are set; Grok uses its own native keys.
        app.session
            .set_claude_enabled_for_test(backend == BackendKind::Codex);
        let mut first = None;
        for event in events {
            handle_server_message_line(&mut app, &event.to_string());
            let id = *first.get_or_insert(app.session.messages()[0].id);
            assert_eq!(app.session.messages().len(), 1, "{backend:?}: {event}");
            assert_eq!(app.session.messages()[0].id, id);
            assert!(matches!(app.item_key("x").item, NativeEntryKey::Item(_)));
            app.assert_entry_invariants();
        }
        assert_eq!(app.session.messages()[0].text, text);
    }
}

fn boundary(method: &str, item: Value) -> Value {
    json!({"method":method,"params":{"turnId":"turn","item":item}})
}

#[test]
fn native_assistant_deltas_keep_one_row_with_mixed_flags() {
    let events = ["a", "b"].map(|delta| {
        json!({"method":"item/agentMessage/delta",
            "params":{"turnId":"turn","itemId":"x","delta":delta}})
    });
    check_stream(&events, "ab");
}

#[test]
fn native_reasoning_completions_keep_one_row_with_mixed_flags() {
    let event = boundary(
        "item/completed",
        json!({"id":"x","type":"reasoning","summary":["why"]}),
    );
    check_stream(&[event.clone(), event], "why");
}

#[test]
fn native_repeated_starts_keep_one_placeholder_with_mixed_flags() {
    let event = boundary("item/started", json!({"id":"x","type":"agentMessage"}));
    check_stream(
        &[
            event.clone(),
            event,
            json!({"method":"item/agentMessage/delta",
                "params":{"turnId":"turn","itemId":"x","delta":"ab"}}),
        ],
        "ab",
    );
}

#[test]
fn native_repeated_read_completions_keep_one_read_with_mixed_flags() {
    let event = boundary(
        "item/completed",
        json!({"id":"x","type":"toolCall","name":"Read","input":{"file_path":"a.rs"}}),
    );
    check_stream(&[event.clone(), event], "→ Read a.rs");
}

#[test]
fn an_unrelated_claude_flag_cannot_unbind_codex_or_grok_native_keys() {
    for backend in [BackendKind::Codex, BackendKind::Grok] {
        for role in [Role::Assistant, Role::Reasoning, Role::ToolCall] {
            for completed in [false, true] {
                let mut app = app(backend);
                let key = app.item_key("x");
                app.session.upsert_mapped_message(
                    &key,
                    role,
                    "original".into(),
                    MessageKind::Plain,
                    None,
                );
                if completed {
                    app.session.mark_item_completed(&key);
                }
                let id = app.session.messages()[0].id;
                app.session.set_claude_enabled_for_test(true);
                for all_parts in [false, true] {
                    app.session.prepare_item_source(&key, None, all_parts);
                    assert_eq!(app.session.item_message_index(&key), Some(0));
                    assert_eq!(app.session.messages()[0].id, id);
                    assert_eq!(app.session.is_item_completed(&key), completed);
                }
            }
        }
    }
}
