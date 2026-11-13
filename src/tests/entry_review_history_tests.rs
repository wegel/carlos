//! Regression cases from the Milestone 2 review, exercised through backend decoders.

use carlos_core::session::models::MessageKind;
use carlos_core::session::models::Role;
use serde_json::{json, Value};

use crate::app::item_history::load_history_from_start_or_resume;
use crate::app::state::AppState;
fn app() -> AppState {
    let mut app = AppState::new("thread".into());
    app.session.set_codex_enabled_for_test(true);
    app.session.enable_native_history();
    app
}
fn load(app: &mut AppState, items: Vec<Value>) {
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
                app.set_rewind_selection_for_test(Some(if last {
                    0
                } else {
                    app.session.input_history().history_len() - 1
                }));
                let request = app.rewind_native_request().expect("native rewind");
                assert_eq!(request.position, if last { 0 } else { hidden_count });
                assert_eq!(request.item_id, "visible");
                let visible = app.rewind_selected_message_idx().expect("visible row");
                assert_eq!(
                    app.session.messages()[visible].kind,
                    if last {
                        MessageKind::Plain
                    } else {
                        MessageKind::UserSteer
                    }
                );
                app.assert_entry_invariants();
            }
        }
    }
}

#[test]
fn review_duplicate_complete_history_keys_cannot_retain_a_unique_id() {
    let mut app = app();
    let row = |text| json!({"type":"agentMessage","id":"same","text":text});
    load(&mut app, vec![row("old")]);
    let old = app.session.messages()[0].id;
    load(&mut app, vec![row("first"), row("second")]);
    assert!(
        app.session.messages().entry(old).is_none(),
        "duplicate source evidence is ambiguous even when the parser collapses display rows"
    );
}
#[test]
fn review_native_override_keeps_heads_generated_label_marker() {
    let mut a = AppState::new("thread".into());
    a.upsert_mapped_message(
        "cmd",
        Role::ToolOutput,
        "body".into(),
        MessageKind::Plain,
        None,
    );
    a.describe_tool_output(
        0,
        &json!({"type":"commandExecution","id":"cmd","command":"python3 process.py"}),
    );
    let id = a.session.messages()[0].id;
    a.session
        .transcript_set_label(id, "run generated".into(), true);
    a.set_command_override("cmd", "run native".into());
    a.view.tool_output.mode = crate::app::tool_output_state::ToolOutputMode::Full;
    a.view.layout.active = crate::app::layout_state::TranscriptTheme::Gutter;
    a.ensure_rendered_lines(80, None);
    let current: Vec<_> = a
        .snapshot_rendered_lines()
        .into_iter()
        .map(|line| line.text)
        .collect();
    let mut head = a.session.messages()[0].tool_output.clone().expect("tool");
    head.generated = true;
    a.session.transcript_set_tool(0, head);
    a.mark_transcript_dirty();
    a.ensure_rendered_lines(80, None);
    let previous: Vec<_> = a
        .snapshot_rendered_lines()
        .into_iter()
        .map(|line| line.text)
        .collect();
    assert_eq!(
        current, previous,
        "HEAD changed only summary text, so the tool label remained visible"
    );
}

#[test]
fn complete_read_history_uses_linear_predecessor_work() {
    let mut counts = Vec::new();
    for count in [1024, 4096] {
        let mut app = app();
        let items: Vec<_> = (0..count).map(|n| json!({"id":format!("read-{n}"),"type":"toolCall","name":"Read","input":{"file_path":"same.rs"}})).collect();
        let thread = json!({"id":"thread","turns":[{"id":"turn","items":items}]});
        let draft = carlos_core::session::item_history_draft::build(&mut app.session, &thread);
        counts.push(draft.read_steps);
        crate::app::item_history::replace_history_from_thread(&mut app, &thread)
            .expect("same-thread history");
        assert_eq!(app.session.messages().len(), count);
        assert!(app.session.messages()[0].text.contains(&count.to_string()));
        assert!(app
            .session
            .messages()
            .iter()
            .skip(1)
            .all(|row| row.text.is_empty()));
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
        let tool = app.session.messages()[0]
            .tool_output
            .as_ref()
            .expect("HEAD retains old tool facts");
        assert_eq!(tool.summary, "Δ Diff a.rs (+1, -1)");
        assert!(tool.failed);
        assert!(!tool.generated);
        assert_eq!(
            app.session.input_history().history_len(),
            0,
            "an existing row is not a new prompt"
        );
        app.assert_entry_invariants();
    }
}

#[test]
fn raw_duplicate_evidence_survives_repeated_loads_and_missing_turn_ids() {
    for turn in [None, Some("turn")] {
        let mut app = app();
        let thread = |texts: &[&str]| json!({"id":"thread","turns":[{"id":turn,"items":texts.iter().map(|text|json!({"type":"agentMessage","id":"same","text":text})).collect::<Vec<_>>()}]});
        crate::app::item_history::replace_history_from_thread(&mut app, &thread(&["old"]))
            .expect("same-thread history");
        let unique = app.session.messages()[0].id;
        crate::app::item_history::replace_history_from_thread(
            &mut app,
            &thread(&["first", "second"]),
        )
        .expect("same-thread history");
        let ambiguous = app.session.messages()[0].id;
        assert_ne!(unique, ambiguous);
        assert_eq!(app.session.messages()[0].text, "second");
        crate::app::item_history::replace_history_from_thread(&mut app, &thread(&["last"]))
            .expect("same-thread history");
        assert_ne!(
            app.session.messages()[0].id,
            ambiguous,
            "old duplicate evidence is still ambiguous"
        );
        app.assert_entry_invariants();
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
    assert_eq!(app.session.messages()[1].text, "→ Read a.rs ×2");
    assert_eq!(app.session.messages()[2].kind, MessageKind::UserSteer);
    assert_eq!(app.session.messages()[3].text, "→ Read a.rs");
    app.assert_entry_invariants();
}

#[test]
#[should_panic]
fn shared_checker_rejects_a_retired_prompt_record() {
    let mut app = app();
    load(
        &mut app,
        vec![json!({"type":"userMessage","id":"user","content":[{"type":"text","text":"prompt"}]})],
    );
    app.session.transcript_reset();
    app.assert_entry_invariants();
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
    app.session.transcript_reset();
    app.assert_entry_invariants();
}
