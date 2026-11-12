//! Prove entry lifetimes through the real app handlers, including hidden and delayed rows.

use serde_json::{json, Value};

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;
use crate::app::{MessageKind, Role};
use crate::backend::BackendKind;
use crate::session::history_identity::{HistoryEntry, HistoryMode};
use crate::session::models::EntryDraft;
use crate::summaries::{command_key, SummaryEvent};

pub(super) fn app(backend: BackendKind) -> AppState {
    let mut app = AppState::new("thread".into());
    app.codex.enabled = backend == BackendKind::Codex;
    app.claude.enabled = backend == BackendKind::Claude;
    app.codex.item_turn = Some("turn".into());
    app.summaries.enabled = true;
    app
}

fn notify(app: &mut AppState, method: &str, item: Value, source: Value) {
    let line = json!({"method":method,"params":{"threadId":app.session.thread_id(),"turnId":"turn",
        "item":item,"sourceKey":source}});
    handle_server_message_line(app, &line.to_string());
}

#[test]
fn placeholder_deltas_completion_role_path_and_labels_keep_the_first_id() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        let mut app = app(backend);
        assert!(app.ensure_item_placeholder("command", Role::ToolCall));
        let id = app.session.messages()[0].id;
        for _ in 0..3 {
            assert!(!app.ensure_item_placeholder("command", Role::ToolCall));
            app.upsert_command_output_delta("command", "delta");
            assert_eq!(app.session.messages()[0].id, id);
        }
        for _ in 0..3 {
            app.upsert_mapped_message(
                "command",
                Role::ToolOutput,
                "final".into(),
                MessageKind::Plain,
                Some("a.rs".into()),
            );
            app.describe_tool_output(
                0,
                &json!({"type":"commandExecution","command":"python3 process.py"}),
            );
            app.set_command_override("command", "label".into());
            app.mark_item_completed("command");
            app.upsert_command_output_delta("command", "late");
            assert_eq!(app.session.messages().len(), 1);
            assert_eq!(app.session.messages()[0].id, id);
            assert_eq!(app.session.messages()[0].text, "final");
            assert_eq!(app.session.messages()[0].file_path.as_deref(), Some("a.rs"));
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
                &index.to_string(),
                Role::ToolCall,
                "→ Read a.rs".into(),
                MessageKind::Plain,
                None,
            );
            ids.push(app.session.messages()[index].id);
        }
        assert_eq!(
            app.session.messages()[0].text,
            format!("→ Read a.rs ×{count}")
        );
        for index in 1..count {
            assert!(app.session.messages()[index].text.is_empty());
            assert_eq!(app.item_message_index(&index.to_string()), Some(index));
            app.upsert_mapped_message(
                &index.to_string(),
                Role::ToolOutput,
                "late shell output".into(),
                MessageKind::Plain,
                None,
            );
            assert_eq!(app.session.messages()[index].id, ids[index]);
            let display = crate::app::tool_output_state::compact_tool_message(
                &app.session.messages()[index],
                0,
            )
            .expect("compact tool");
            assert_eq!(display.source.id, ids[index]);
        }
        assert_eq!(app.session.messages()[0].id, ids[0]);
    }
}

#[test]
fn diff_parts_do_not_alias_literal_suffix_ids() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for count in 1..5 {
            let mut app = app(backend);
            app.upsert_mapped_message(
                "x#part-1",
                Role::Assistant,
                "literal".into(),
                MessageKind::Plain,
                None,
            );
            let literal = app.session.messages()[0].id;
            let mut ids = Vec::new();
            for part in 0..count {
                app.with_item_part("x", part, |app, key| {
                    app.upsert_mapped_message(
                        key,
                        Role::ToolOutput,
                        "diff".into(),
                        MessageKind::Diff,
                        Some(part.to_string()),
                    );
                    ids.push(app.session.messages()[app.item_message_index(key).expect("part")].id);
                });
            }
            for retained in 0..=count {
                app.session
                    .transcript_retain_parts(&app.item_key("x"), retained);
                for part in 0..count {
                    app.with_item_part("x", part, |app, key| {
                        assert_eq!(app.has_item_mapping(key), part < retained);
                    });
                }
                if retained == 0 {
                    break;
                }
            }
            app.with_item_part("x", count - 1, |app, key| {
                app.upsert_mapped_message(
                    key,
                    Role::ToolOutput,
                    "new".into(),
                    MessageKind::Diff,
                    None,
                );
                let new = app.session.messages()[app.item_message_index(key).expect("new part")].id;
                assert!(!ids.contains(&new));
            });
            assert_eq!(
                app.session.messages()[app.item_message_index("x#part-1").expect("literal")].id,
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
        let old = app.session.messages()[0].id;
        notify(
            &mut app,
            "item/completed",
            json!({"id":"claude-msg-0-1-0","type":"agentMessage","text":"final"}),
            source,
        );
        assert_eq!(app.session.messages()[0].id, old);
        let native = app
            .session
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
        app.session
            .transcript_replace_history(vec![row], HistoryMode::SameThread);
        assert_eq!(
            app.session.messages()[0].id == old,
            matches!(kind, "assistant" | "tool" | "user")
        );
    }
}

#[test]
fn stale_summary_cannot_write_to_a_replacement_entry() {
    for change in ["reload", "insert", "correct", "fork", "truncate"] {
        let mut app = app(BackendKind::Grok);
        let command = json!({"id":"command","type":"commandExecution","command":"python3 process.py","aggregatedOutput":"out"});
        notify(&mut app, "item/completed", command.clone(), Value::Null);
        let id = app.session.messages()[0].id;
        let token = app.summary_token_for_test("command");
        if change == "fork" {
            app.reset_for_forked_thread("fork");
        }
        if change == "truncate" {
            app.rewind_fork_from_message_idx(Some(0));
        }
        let prefix = if change == "insert" {
            vec![json!({"id":"new","type":"agentMessage","text":"before"})]
        } else {
            vec![]
        };
        let mut items = prefix;
        let mut command = command;
        if change == "correct" {
            command["aggregatedOutput"] = json!("corrected");
        }
        items.push(command);
        let response = json!({"result":{"thread":{"id":app.session.thread_id(),"turns":[{"id":"turn","items":items}]}}});
        load_history_from_start_or_resume(&mut app, &response.to_string()).expect("history");
        app.handle_summary_event(SummaryEvent::Ready(
            command_key("python3 process.py"),
            token,
            Some("STALE".into()),
        ));
        let index = app.item_message_index("command").expect("reloaded command");
        assert!(
            !app.session.messages()[index]
                .tool_output
                .as_ref()
                .expect("tool")
                .generated,
            "{change}"
        );
        assert_eq!(
            app.session.messages()[index].id == id,
            !matches!(change, "fork" | "truncate")
        );
        let token = app.summary_token_for_test("command");
        app.handle_summary_event(SummaryEvent::Ready(
            command_key("python3 process.py"),
            token,
            Some("fresh".into()),
        ));
        assert!(
            app.session.messages()[index]
                .tool_output
                .as_ref()
                .expect("tool")
                .generated
        );
    }
}

#[test]
fn equal_local_echoes_bind_only_their_returned_uuids_and_keep_distinct_ids() {
    let mut app = app(BackendKind::Claude);
    let mut ids = Vec::new();
    for uuid in ["first", "second", "third"] {
        let index = app.append_message(Role::User, "same");
        ids.push(app.session.messages()[index].id);
        app.record_input_history("same", Some(index));
        crate::app::claude_prompts::attach_echo(&mut app, uuid, None);
    }
    assert_eq!(app.input_history_len(), 3);
    for (index, uuid) in ["first", "second", "third"].into_iter().enumerate() {
        let key = app.item_key(&format!("claude-prompt:{uuid}"));
        assert_eq!(
            app.session.messages().entry_id_for_item(&key),
            Some(ids[index])
        );
        app.set_rewind_selection_for_test(Some(index));
        assert_eq!(app.rewind_selected_turn_id(), Some(uuid));
    }
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
}

#[test]
fn complete_diff_changes_keep_existing_parts_and_remint_reintroduced_parts() {
    for backend in [BackendKind::Codex, BackendKind::Claude, BackendKind::Grok] {
        for count in 1..5 {
            for retained in 1..=count {
                let mut app = app(backend);
                let item = |count: usize, text: &str| {
                    json!({"id":"diff","type":"toolOutput",
                    "files":(0..count).map(|part| json!({"filePath":format!("{part}.rs"),
                        "diff":format!("@@ -1 +1 @@\n-old\n+{text}\n")})).collect::<Vec<_>>()})
                };
                let source = json!({"kind":"tool","id":"native","part":"result"});
                notify(
                    &mut app,
                    "item/completed",
                    item(count, "first"),
                    source.clone(),
                );
                let old: Vec<_> = app.session.messages().iter().map(|row| row.id).collect();
                notify(
                    &mut app,
                    "item/completed",
                    item(retained, "changed"),
                    source.clone(),
                );
                for part in 0..count {
                    app.with_item_part("diff", part, |app, key| {
                        assert_eq!(app.has_item_mapping(key), part < retained);
                        if part < retained {
                            assert_eq!(
                                app.session.messages()
                                    [app.item_message_index(key).expect("retained")]
                                .id,
                                old[part]
                            );
                        }
                    });
                }
                notify(&mut app, "item/completed", item(count + 1, "added"), source);
                for part in 0..=count {
                    app.with_item_part("diff", part, |app, key| {
                        let id = app.session.messages()
                            [app.item_message_index(key).expect("current part")]
                        .id;
                        assert_eq!(old.contains(&id), part < retained);
                    });
                }
            }
        }
    }
}

#[test]
fn a_new_claude_cursor_cannot_append_to_completed_native_work() {
    for source in [
        json!({"kind":"assistant","id":"native","block":0}),
        json!({"kind":"tool","id":"native","part":"result"}),
    ] {
        let mut app = app(BackendKind::Claude);
        notify(
            &mut app,
            "item/completed",
            json!({"id":"cursor-a","type":"agentMessage","text":"final"}),
            source.clone(),
        );
        let id = app.session.messages()[0].id;
        let delta = json!({"method":"item/agentMessage/delta","params":{
            "threadId":"thread","itemId":"cursor-b","delta":" late duplicate","sourceKey":source}});
        handle_server_message_line(&mut app, &delta.to_string());
        assert_eq!(app.session.messages().len(), 1);
        assert_eq!(app.session.messages()[0].id, id);
        assert_eq!(app.session.messages()[0].text, "final");
    }
}
