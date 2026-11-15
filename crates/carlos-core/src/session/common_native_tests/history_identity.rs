//! Native reloads update ids, prompt references and view caches together.

use crate::session::models::Role;
use serde_json::json;

use super::session;
use crate::session::item_history::load_history_from_start_or_resume;
use crate::session::state::SessionState;

#[test]
fn native_reload_retains_turn_scoped_ids_and_rewind_targets_after_insertions() {
    for insertion in 0..3 {
        let mut app = session("thread");
        app.set_codex_enabled_for_test(true);
        app.enable_native_history();
        let turn = |name: &str, answer: &str| {
            json!({"id":name,"status":"completed","items":[
            {"type":"userMessage","id":"same-user","content":[{"type":"text","text":"same"}]},
            {"type":"agentMessage","id":"same-answer","text":answer}]})
        };
        let old = json!({"result":{"thread":{"id":"thread","turns":[turn("a", "old a"), turn("b", "old b")]}}});
        load_history_from_start_or_resume(&mut app, &old.to_string()).expect("initial history");
        let ids: Vec<_> = app.messages().iter().map(|row| row.id).collect();
        let mut turns = vec![turn("a", "corrected a"), turn("b", "corrected b")];
        turns.insert(insertion, turn("x", "discovered"));
        let new = json!({"result":{"thread":{"id":"thread","turns":turns}}});
        load_history_from_start_or_resume(&mut app, &new.to_string()).expect("reload");
        for (old_turn, name) in ["a", "b"].into_iter().enumerate() {
            let position = (old_turn + usize::from(insertion <= old_turn)) * 2;
            assert_eq!(app.messages()[position].id, ids[old_turn * 2]);
            assert_eq!(app.messages()[position + 1].id, ids[old_turn * 2 + 1]);
            assert!(app.messages()[position + 1].text.starts_with("corrected"));
            let entry = app
                .input_history()
                .entry_id(position / 2)
                .expect("prompt entry");
            assert_eq!(app.messages().position(entry), Some(position));
            assert_eq!(app.input_history().turn_id(entry), Some(name));
            let request = app
                .input_history()
                .native_request(position / 2, entry)
                .expect("native rewind");
            assert_eq!(request.item_id, "same-user");
            assert_eq!(request.position, 0);
        }
    }
}

#[test]
fn history_prompts_notices_and_unknown_authors_get_distinct_carlos_ids() {
    let mut app = session("thread");
    app.set_claude_enabled_for_test(true);
    for source in ["human", "unknown", "automatic"] {
        for suffix in ["a", "b"] {
            let facts = json!({"source":source,"uuid":format!("{source}-{suffix}"),"text":"same"});
            let uuid = facts["uuid"].as_str().expect("native prompt");
            let key = app.backend_entry_key(&format!("claude-prompt:{uuid}"));
            app.append_claude_history_prompt(&facts, &key);
            let count = app.messages().len();
            let id = app.messages()[count - 1].id;
            let uuid = facts["uuid"].as_str().expect("native prompt");
            let key = app.backend_entry_key(&format!("claude-prompt:{uuid}"));
            app.append_claude_history_prompt(&facts, &key);
            assert_eq!(app.messages().len(), count);
            assert_eq!(app.messages()[count - 1].id, id);
        }
    }
    app.append_message(Role::System, "notice");
    let ids: std::collections::HashSet<_> = app.messages().iter().map(|row| row.id).collect();
    assert_eq!(ids.len(), 7);
    assert_eq!(
        app.claude_history().unknown()[0].entry_id(),
        app.messages()[2].id
    );
    assert_eq!(
        app.claude_history().unknown()[1].entry_id(),
        app.messages()[3].id
    );
}

#[test]
fn complete_history_drafts_keep_the_same_read_groups_as_incremental_history() {
    for length in 0..5 {
        for mut sequence in 0..3_usize.pow(length) {
            let mut items = Vec::new();
            for _ in 0..length {
                let key = ["a", "b", "c"][sequence % 3];
                sequence /= 3;
                items.push(
                    json!({"type":"toolCall","id":key,"name":"Read","input":{"file_path":"a.rs"}}),
                );
            }
            let thread = json!({"id":"thread","turns":[{"id":"turn","items":items}]});
            let mut reference = super::head_reference::HeadReference::default();
            for item in thread["turns"][0]["items"].as_array().expect("items") {
                reference.upsert_mapped_message(
                    item["id"].as_str().expect("id"),
                    Role::ToolCall,
                    "→ Read a.rs".into(),
                    crate::session::models::MessageKind::Plain,
                    Some("a.rs".into()),
                );
            }
            let mut reloaded = session("thread");
            crate::session::item_history::replace_history_from_thread(&mut reloaded, &thread)
                .expect("same-thread history");
            let rows = |app: &SessionState| {
                app.messages()
                    .iter()
                    .map(|row| (row.role, row.kind, row.text.clone(), row.file_path.clone()))
                    .collect::<Vec<_>>()
            };
            let expected: Vec<_> = reference
                .messages
                .iter()
                .map(|row| (row.role, row.kind, row.text.clone(), row.file_path.clone()))
                .collect();
            assert_eq!(expected, rows(&reloaded));
            reloaded.messages().assert_invariants();
        }
    }
}
