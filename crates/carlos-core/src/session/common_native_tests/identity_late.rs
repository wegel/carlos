//! Reject stale labels and completed deltas while preserving prompt and diff identities.

use super::feed as handle_server_message_line;
use super::identity_support::{notify, reset, session_for as app, summary_token, truncate};
use crate::backend::BackendKind;
use crate::session::item_history::load_history_from_start_or_resume;
use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::summaries::{command_key, SummaryEvent};
use serde_json::{json, Value};

#[test]
fn stale_summary_cannot_write_to_a_replacement_entry() {
    for change in ["reload", "insert", "correct", "fork", "truncate"] {
        let mut app = app(BackendKind::Grok);
        let command = json!({"id":"command","type":"commandExecution","command":"python3 process.py","aggregatedOutput":"out"});
        notify(&mut app, "item/completed", command.clone(), Value::Null);
        let id = app.messages()[0].id;
        let token = summary_token(&app, "command");
        reload_command(&mut app, command, change);
        app.handle_summary_event(SummaryEvent::Ready(
            command_key("python3 process.py"),
            token,
            Some("STALE".into()),
        ));
        let index = app
            .item_message_index(&app.backend_entry_key("command"))
            .expect("reloaded command");
        assert!(
            !app.messages()[index]
                .tool_output
                .as_ref()
                .expect("tool")
                .generated,
            "{change}"
        );
        assert_eq!(
            app.messages()[index].id == id,
            !matches!(change, "fork" | "truncate")
        );
        let token = summary_token(&app, "command");
        app.handle_summary_event(SummaryEvent::Ready(
            command_key("python3 process.py"),
            token,
            Some("fresh".into()),
        ));
        assert!(
            app.messages()[index]
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
        ids.push(app.messages()[index].id);
        app.record_prompt_history("same", Some(app.messages()[index].id));
        let key = app.backend_entry_key(&format!("claude-prompt:{uuid}"));
        app.attach_claude_prompt_echo(uuid, &key, None);
    }
    assert_eq!(app.input_history().history_len(), 3);
    for (index, uuid) in ["first", "second", "third"].into_iter().enumerate() {
        let key = app.backend_entry_key(&format!("claude-prompt:{uuid}"));
        assert_eq!(app.messages().entry_id_for_item(&key), Some(ids[index]));
        assert_eq!(app.input_history().entry_id(index), Some(ids[index]));
        assert_eq!(app.input_history().turn_id(ids[index]), Some(uuid));
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
                let old: Vec<_> = app.messages().iter().map(|row| row.id).collect();
                notify(
                    &mut app,
                    "item/completed",
                    item(retained, "changed"),
                    source.clone(),
                );
                for (part, id) in old.iter().enumerate().take(count) {
                    app.with_native_item_part("diff", part, |app, key| {
                        assert_eq!(
                            app.has_item_mapping(&app.backend_entry_key(key)),
                            part < retained
                        );
                        if part < retained {
                            assert_eq!(
                                app.messages()[app
                                    .item_message_index(&app.backend_entry_key(key))
                                    .expect("retained")]
                                .id,
                                *id
                            );
                        }
                    });
                }
                notify(&mut app, "item/completed", item(count + 1, "added"), source);
                for part in 0..=count {
                    app.with_native_item_part("diff", part, |app, key| {
                        let id = app.messages()[app
                            .item_message_index(&app.backend_entry_key(key))
                            .expect("current part")]
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
        let id = app.messages()[0].id;
        let delta = json!({"method":"item/agentMessage/delta","params":{
            "threadId":"thread","itemId":"cursor-b","delta":" late duplicate","sourceKey":source}});
        handle_server_message_line(&mut app, &delta.to_string());
        assert_eq!(app.messages().len(), 1);
        assert_eq!(app.messages()[0].id, id);
        assert_eq!(app.messages()[0].text, "final");
    }
}

fn reload_command(app: &mut SessionState, command: Value, change: &str) {
    if change == "fork" {
        reset(app, "fork");
    }
    if change == "truncate" {
        truncate(app, 0);
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
    let response =
        json!({"result":{"thread":{"id":app.thread_id(),"turns":[{"id":"turn","items":items}]}}});
    load_history_from_start_or_resume(app, &response.to_string()).expect("history");
}
