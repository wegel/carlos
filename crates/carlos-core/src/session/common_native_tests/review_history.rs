//! Regression cases from the Milestone 2 review, exercised through backend decoders.

use serde_json::{json, Value};

use super::identity_support::assert_entries;
use super::session;
use crate::session::item_history::{
    load_history_from_start_or_resume, replace_history_from_thread,
};
use crate::session::models::{MessageKind, Role};
use crate::session::state::SessionState;

fn app() -> SessionState {
    let mut app = session("thread");
    app.set_codex_enabled_for_test(true);
    app.enable_native_history();
    app
}
fn load(app: &mut SessionState, items: Vec<Value>) {
    let value = json!({"result":{"thread":{"id":"thread","turns":[{"id":"turn","status":"completed","items":items}]}}});
    load_history_from_start_or_resume(app, &value.to_string()).expect("history");
}
#[test]
fn review_hidden_native_user_must_count_toward_rewind_position() {
    for hidden in [
        json!({"content": [{"type":"image","imageUrl":"attachment"}]}),
        json!({"content": []}),
        json!({}),
        json!({"content": [{"type":"text","text":"   \n"}]}),
    ] {
        for hidden_count in 1..5 {
            for last in [false, true] {
                let mut app = app();
                let mut items = Vec::new();
                for n in 0..hidden_count {
                    let mut item = hidden.clone();
                    item["type"] = json!("userMessage");
                    item["id"] = json!(format!("hidden-{n}"));
                    items.push(item);
                }
                let visible = json!({"type":"userMessage","id":"visible","content":[{"type":"text","text":"visible"}]});
                items.insert(if last { 0 } else { hidden_count }, visible);
                load(&mut app, items);
                let history = if last {
                    0
                } else {
                    app.input_history().history_len() - 1
                };
                let entry = app
                    .input_history()
                    .entry_id(history)
                    .expect("visible prompt");
                let request = app
                    .input_history()
                    .native_request(history, entry)
                    .expect("native rewind");
                assert_eq!(request.position, if last { 0 } else { hidden_count });
                assert_eq!(request.item_id, "visible");
                let visible = app.messages().position(entry).expect("visible row");
                assert_eq!(
                    app.messages()[visible].kind,
                    if last {
                        MessageKind::Plain
                    } else {
                        MessageKind::UserSteer
                    }
                );
                assert_entries(&app);
            }
        }
    }
}

#[test]
fn review_duplicate_complete_history_keys_cannot_retain_a_unique_id() {
    let mut app = app();
    let row = |text| json!({"type":"agentMessage","id":"same","text":text});
    load(&mut app, vec![row("old")]);
    let old = app.messages()[0].id;
    load(&mut app, vec![row("first"), row("second")]);
    assert!(
        app.messages().entry(old).is_none(),
        "duplicate source evidence is ambiguous even when the parser collapses display rows"
    );
}
#[test]
fn review_native_override_keeps_heads_generated_label_marker() {
    let mut a = session("thread");
    let key = a.backend_entry_key("cmd");
    a.upsert_mapped_message(
        &key,
        Role::ToolOutput,
        "body".into(),
        MessageKind::Plain,
        None,
    );
    a.describe_tool_output(
        0,
        &json!({"type":"commandExecution","id":"cmd","command":"python3 process.py"}),
        Some(&key),
    );
    let id = a.messages()[0].id;
    a.transcript_set_label(id, "run generated".into(), true);
    a.set_command_override(&key, "run native".into());
    let tool = a.messages()[0].tool_output.as_ref().expect("tool facts");
    assert_eq!(tool.summary, "run native");
    assert!(
        tool.generated,
        "native override preserves the generated-label marker"
    );
    assert_eq!(a.messages()[0].text, "body");
}

#[test]
fn complete_read_history_uses_linear_predecessor_work() {
    let mut counts = Vec::new();
    for count in [1024, 4096] {
        let mut app = app();
        let items: Vec<_> = (0..count).map(|n| json!({"id":format!("read-{n}"),"type":"toolCall","name":"Read","input":{"file_path":"same.rs"}})).collect();
        let thread = json!({"id":"thread","turns":[{"id":"turn","items":items}]});
        let draft = crate::session::item_history_draft::build(&mut app, &thread);
        counts.push(draft.read_steps);
        replace_history_from_thread(&mut app, &thread).expect("same-thread history");
        assert_eq!(app.messages().len(), count);
        assert!(app.messages()[0].text.contains(&count.to_string()));
        assert!(app.messages().iter().skip(1).all(|row| row.text.is_empty()));
    }
    println!("read predecessor steps: {counts:?}");
    assert!(counts[1] <= counts[0] * 4 + 32);
    assert!(counts[1] < 4096 * 8);
}

#[test]
fn history_keeps_tool_facts_and_records_only_new_user_rows_at_the_event() {
    for replacement in ["userMessage", "agentMessage", "toolCall"] {
        let mut app = app();
        let diff = json!({"type":"toolOutput","id":"same","files":[{"filePath":"a.rs","diff":"@@ -1 +1 @@\n-old\n+new"}],"exitCode":2});
        let later = json!({"type":replacement,"id":"same","text":"later","content":[{"type":"text","text":"later"}],"name":"Read","input":{"file_path":"a.rs"}});
        load(&mut app, vec![diff, later]);
        let tool = app.messages()[0]
            .tool_output
            .as_ref()
            .expect("HEAD retains old tool facts");
        assert_eq!(tool.summary, "Δ Diff a.rs (+1, -1)");
        assert!(tool.failed);
        assert!(!tool.generated);
        assert_eq!(
            app.input_history().history_len(),
            0,
            "an existing row is not a new prompt"
        );
        assert_entries(&app);
    }
}

#[test]
fn raw_duplicate_evidence_survives_repeated_loads_and_missing_turn_ids() {
    for turn in [None, Some("turn")] {
        let mut app = app();
        let thread = |texts: &[&str]| json!({"id":"thread","turns":[{"id":turn,"items":texts.iter().map(|text|json!({"type":"agentMessage","id":"same","text":text})).collect::<Vec<_>>()}]});
        replace_history_from_thread(&mut app, &thread(&["old"])).expect("same-thread history");
        let unique = app.messages()[0].id;
        replace_history_from_thread(&mut app, &thread(&["first", "second"]))
            .expect("same-thread history");
        let ambiguous = app.messages()[0].id;
        assert_ne!(unique, ambiguous);
        assert_eq!(app.messages()[0].text, "second");
        replace_history_from_thread(&mut app, &thread(&["last"])).expect("same-thread history");
        assert_ne!(
            app.messages()[0].id,
            ambiguous,
            "old duplicate evidence is still ambiguous"
        );
        assert_entries(&app);
    }
}

#[test]
fn an_invisible_steer_on_a_read_shell_blocks_later_read_grouping() {
    let mut app = app();
    let read = |id| json!({"type":"toolCall","id":id,"name":"Read","input":{"file_path":"a.rs"}});
    let items = vec![
        json!({"type":"userMessage","id":"first","content":[{"type":"text","text":"first"}]}),
        read("a"),
        read("b"),
        json!({"type":"userMessage","id":"b","content":[]}),
        read("c"),
    ];
    load(&mut app, items);
    assert_eq!(app.messages()[1].text, "→ Read a.rs ×2");
    assert_eq!(app.messages()[2].kind, MessageKind::UserSteer);
    assert_eq!(app.messages()[3].text, "→ Read a.rs");
    assert_entries(&app);
}

#[test]
#[should_panic]
fn shared_checker_rejects_a_retired_prompt_record() {
    let mut app = app();
    load(
        &mut app,
        vec![json!({"type":"userMessage","id":"user","content":[{"type":"text","text":"prompt"}]})],
    );
    assert_entries(&app);
    app.transcript_reset();
    assert_entries(&app);
}

#[test]
#[should_panic]
fn shared_checker_rejects_a_retired_summary_binding() {
    let mut app = app();
    load(
        &mut app,
        vec![
            json!({"type":"commandExecution","id":"cmd","command":"python3 process.py","aggregatedOutput":"body"}),
        ],
    );
    assert_entries(&app);
    app.transcript_reset();
    assert_entries(&app);
}
