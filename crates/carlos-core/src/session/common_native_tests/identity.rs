//! Keep original entry ids across native aliases, hidden read rows and diff parts.

use super::identity_support::{notify, session_for as app};
use crate::backend::BackendKind;
use crate::session::history_identity::{HistoryEntry, HistoryMode};
use crate::session::models::{EntryDraft, MessageKind, Role};
use serde_json::{json, Value};

#[test]
fn placeholder_deltas_completion_role_path_and_labels_keep_the_first_id() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        let mut app = app(backend);
        assert!(app.ensure_item_placeholder(&app.backend_entry_key("command"), Role::ToolCall));
        let id = app.messages()[0].id;
        for _ in 0..3 {
            assert!(!app.ensure_item_placeholder(&app.backend_entry_key("command"), Role::ToolCall));
            app.upsert_command_output_delta(&app.backend_entry_key("command"), "delta");
            assert_eq!(app.messages()[0].id, id);
        }
        for _ in 0..3 {
            app.upsert_mapped_message(
                &app.backend_entry_key("command"),
                Role::ToolOutput,
                "final".into(),
                MessageKind::Plain,
                Some("a.rs".into()),
            );
            app.describe_tool_output(
                0,
                &json!({"type":"commandExecution","command":"python3 process.py"}),
                None,
            );
            app.set_command_override(&app.backend_entry_key("command"), "label".into());
            app.mark_item_completed(&app.backend_entry_key("command"));
            app.upsert_command_output_delta(&app.backend_entry_key("command"), "late");
            assert_eq!(app.messages().len(), 1);
            assert_eq!(app.messages()[0].id, id);
            assert_eq!(app.messages()[0].text, "final");
            assert_eq!(app.messages()[0].file_path.as_deref(), Some("a.rs"));
        }
    }
}

#[test]
fn read_coalescing_keeps_the_hidden_shell_id() {
    for count in 2..6 {
        let mut app = app(BackendKind::Grok);
        let mut ids = Vec::new();
        for index in 0..count {
            app.upsert_mapped_message(
                &app.backend_entry_key(&index.to_string()),
                Role::ToolCall,
                "→ Read a.rs".into(),
                MessageKind::Plain,
                None,
            );
            ids.push(app.messages()[index].id);
        }
        assert_eq!(app.messages()[0].text, format!("→ Read a.rs ×{count}"));
        for (index, id) in ids.iter().enumerate().take(count).skip(1) {
            assert!(app.messages()[index].text.is_empty());
            assert_eq!(
                app.item_message_index(&app.backend_entry_key(&index.to_string())),
                Some(index)
            );
            app.upsert_mapped_message(
                &app.backend_entry_key(&index.to_string()),
                Role::ToolOutput,
                "late shell output".into(),
                MessageKind::Plain,
                None,
            );
            assert_eq!(app.messages()[index].id, ids[index]);
            assert_eq!(app.messages()[index].id, *id);
        }
        assert_eq!(app.messages()[0].id, ids[0]);
    }
}

#[test]
fn diff_parts_do_not_alias_literal_suffix_ids() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for count in 1..5 {
            let mut app = app(backend);
            app.upsert_mapped_message(
                &app.backend_entry_key("x#part-1"),
                Role::Assistant,
                "literal".into(),
                MessageKind::Plain,
                None,
            );
            let literal = app.messages()[0].id;
            let mut ids = Vec::new();
            for part in 0..count {
                app.with_native_item_part("x", part, |app, key| {
                    app.upsert_mapped_message(
                        &app.backend_entry_key(key),
                        Role::ToolOutput,
                        "diff".into(),
                        MessageKind::Diff,
                        Some(part.to_string()),
                    );
                    ids.push(
                        app.messages()[app
                            .item_message_index(&app.backend_entry_key(key))
                            .expect("part")]
                        .id,
                    );
                });
            }
            for retained in 0..=count {
                app.transcript_retain_parts(&app.backend_entry_key("x"), retained);
                for part in 0..count {
                    app.with_native_item_part("x", part, |app, key| {
                        assert_eq!(
                            app.has_item_mapping(&app.backend_entry_key(key)),
                            part < retained
                        );
                    });
                }
                if retained == 0 {
                    break;
                }
            }
            app.with_native_item_part("x", count - 1, |app, key| {
                app.upsert_mapped_message(
                    &app.backend_entry_key(key),
                    Role::ToolOutput,
                    "new".into(),
                    MessageKind::Diff,
                    None,
                );
                let new = app.messages()[app
                    .item_message_index(&app.backend_entry_key(key))
                    .expect("new part")]
                .id;
                assert!(!ids.contains(&new));
            });
            assert_eq!(
                app.messages()[app
                    .item_message_index(&app.backend_entry_key("x#part-1"))
                    .expect("literal")]
                .id,
                literal
            );
            assert!(ids.iter().all(|id| *id != literal));
        }
    }
}

#[test]
fn claude_native_evidence_matches_live_and_history_but_generated_counters_do_not() {
    for kind in [
        "assistant",
        "tool",
        "user",
        "missing",
        "no-block",
        "bad-part",
    ] {
        let mut app = app(BackendKind::Claude);
        let source = match kind {
            "assistant" => json!({"kind":kind,"id":"native","block":2,"sessionId":"thread"}),
            "tool" => json!({"kind":kind,"id":"native","part":"result","sessionId":"thread"}),
            "user" => json!({"kind":kind,"id":"native","sessionId":"thread"}),
            "no-block" => json!({"kind":"assistant","id":"native"}),
            "bad-part" => json!({"kind":"tool","id":"native","part":"unknown"}),
            _ => Value::Null,
        };
        notify(
            &mut app,
            "item/started",
            json!({"id":"claude-msg-0-1-0","type":"agentMessage"}),
            source.clone(),
        );
        let old = app.messages()[0].id;
        notify(
            &mut app,
            "item/completed",
            json!({"id":"claude-msg-0-1-0","type":"agentMessage","text":"final"}),
            source,
        );
        assert_eq!(app.messages()[0].id, old);
        let native = app
            .messages()
            .items()
            .keys()
            .find(|key| key.durable())
            .cloned();
        let row = HistoryEntry {
            occurrences: 1,
            draft: EntryDraft::plain(Role::Assistant, "corrected"),
            key: native,
        };
        app.transcript_replace_history(vec![row], HistoryMode::SameThread);
        assert_eq!(
            app.messages()[0].id == old,
            matches!(kind, "assistant" | "tool" | "user")
        );
    }
}
