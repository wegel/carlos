//! Keep native Codex and Grok rows bound when Claude has no evidence about them.

use crate::backend::BackendKind;
use crate::session::entry_key::NativeEntryKey;
use crate::session::models::{MessageKind, Role};
use serde_json::{json, Value};

use super::feed as handle_server_message_line;
use super::identity_support::{assert_entries, session_for as app};

fn check_stream(events: &[Value], text: &str) {
    for backend in [BackendKind::Codex, BackendKind::Grok] {
        let mut app = app(backend);
        app.set_summary_enabled_for_test(false);
        // Codex wins when both flags are set; Grok uses its own native keys.
        app.set_claude_enabled_for_test(backend == BackendKind::Codex);
        let mut first = None;
        for event in events {
            handle_server_message_line(&mut app, &event.to_string());
            let id = *first.get_or_insert(app.messages()[0].id);
            assert_eq!(app.messages().len(), 1, "{backend:?}: {event}");
            assert_eq!(app.messages()[0].id, id);
            assert!(matches!(
                app.backend_entry_key("x").item,
                NativeEntryKey::Item(_)
            ));
            assert_entries(&app);
        }
        assert_eq!(app.messages()[0].text, text);
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
                let key = app.backend_entry_key("x");
                app.upsert_mapped_message(&key, role, "original".into(), MessageKind::Plain, None);
                if completed {
                    app.mark_item_completed(&key);
                }
                let id = app.messages()[0].id;
                app.set_claude_enabled_for_test(true);
                for all_parts in [false, true] {
                    app.prepare_item_source(&key, None, all_parts);
                    assert_eq!(app.item_message_index(&key), Some(0));
                    assert_eq!(app.messages()[0].id, id);
                    assert_eq!(app.is_item_completed(&key), completed);
                }
            }
        }
    }
}
