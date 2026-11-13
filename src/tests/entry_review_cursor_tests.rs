//! Regression cases from the Milestone 2 review, exercised through backend decoders.

use carlos_core::session::models::Role;
use serde_json::json;

use crate::app::notifications::handle_server_message_line;
use crate::app::state::AppState;

#[test]
fn review_cursor_aliases_stay_bounded_for_one_native_entry() {
    let mut app = AppState::new("thread".into());
    app.session.set_claude_enabled_for_test(true);
    for cursor in 0..1000 {
        let line = json!({"method":"item/completed","params":{"threadId":"thread","item":{"id":format!("claude-msg-{cursor}-0-0"),"type":"agentMessage","text":"same"},"sourceKey":{"kind":"assistant","id":"native","block":0,"sessionId":"thread"}}});
        handle_server_message_line(&mut app, &line.to_string());
    }
    assert_eq!(app.session.messages().len(), 1);
    assert_eq!(app.session.messages().alias_sizes(), (2, 2, 2));
    assert_eq!(app.session.completed_item_ids().len(), 1);
    app.assert_entry_invariants();
    assert!(
        app.session.messages().items().len() < 10,
        "one native row accumulated {} live aliases",
        app.session.messages().items().len()
    );
}

#[test]
fn review_real_history_cursor_aliases_are_bounded() {
    let mut a = AppState::new("thread".into());
    a.session.set_claude_enabled_for_test(true);
    a.session.set_claude_generation_for_test(1);
    let mut backend = carlos_core::claude_backend::FixtureNormalizer::default();
    let input = json!({"method":"claude/controller","params":{"generation":1,"epoch":0,"requestMethod":"sessions.read","requestContext":{},"frame":{"type":"reply","ok":true,"result":{"messages":[{"source":"system","message":{"type":"assistant","uuid":"native-snapshot","session_id":"thread","message":{"id":"native","role":"assistant","content":[{"type":"text","text":"same"}]}}}],"nextOffset":null}}}});
    for _ in 0..1000 {
        for line in backend.lines(&input, "thread", 1) {
            handle_server_message_line(&mut a, &line);
        }
    }
    assert_eq!(a.session.messages().len(), 1);
    a.assert_entry_invariants();
    assert_eq!(a.session.messages().alias_sizes(), (2, 2, 2));
    assert_eq!(a.session.completed_item_ids().len(), 1);
    assert!(
        a.session.messages().items().len() < 10,
        "actual backend repeated native history left {} aliases for one row",
        a.session.messages().items().len()
    );
}

#[test]
fn review_generated_cursor_name_cannot_merge_a_native_tool_with_reasoning() {
    let mut a = AppState::new("thread".into());
    a.session.set_claude_enabled_for_test(true);
    a.session.set_claude_generation_for_test(1);
    let mut backend = carlos_core::claude_backend::FixtureNormalizer::default();
    let input = json!({"method":"claude/controller","params":{"generation":1,"epoch":0,"requestMethod":"sessions.read","requestContext":{},"frame":{"type":"reply","ok":true,"result":{"messages":[{"source":"system","message":{"type":"assistant","session_id":"thread","uuid":"snapshot","message":{"id":"native","role":"assistant","content":[{"type":"thinking","thinking":"why"},{"type":"tool_use","id":"claude-msg-0-2-0","name":"Bash","input":{"command":"pwd"}}]}}}],"nextOffset":null}}}});
    for line in backend.lines(&input, "thread", 1) {
        handle_server_message_line(&mut a, &line);
    }
    assert_eq!(
        a.session.messages().len(),
        2,
        "native tool identity and generated reasoning cursor are distinct"
    );
    assert_eq!(a.session.messages()[0].role, Role::Reasoning);
    assert_eq!(a.session.messages()[0].text, "why");
    assert_ne!(a.session.messages()[0].id, a.session.messages()[1].id);
}

fn native_lines(
    app: &mut AppState,
    backend: &mut carlos_core::claude_backend::FixtureNormalizer,
    record: serde_json::Value,
    live: bool,
) -> Vec<serde_json::Value> {
    let frame = if live {
        json!({"v":1,"type":"event","event":"sdk","message":record})
    } else {
        json!({"type":"reply","ok":true,"result":{"messages":[{"source":"system","message":record}],"nextOffset":null}})
    };
    let input = json!({"method":"claude/controller","params":{"generation":1,"epoch":0,"requestMethod":"sessions.read","requestContext":{},"frame":frame}});
    backend
        .lines(&input, "thread", 1)
        .into_iter()
        .map(|line| {
            handle_server_message_line(app, &line);
            serde_json::from_str(&line).expect("decoded wire")
        })
        .collect()
}

#[test]
fn native_tool_ids_cannot_adopt_an_unproven_assistant_cursor_in_live_or_history() {
    for live in [false, true] {
        for proven in [false, true] {
            let mut app = AppState::new("thread".into());
            app.session.set_claude_enabled_for_test(true);
            app.session.set_claude_generation_for_test(1);
            let mut backend = carlos_core::claude_backend::FixtureNormalizer::default();
            let mut record = json!({"type":"assistant","session_id":"thread","message":{"role":"assistant","content":[{"type":"thinking","thinking":"why"}]}});
            if proven {
                record["message"]["id"] = json!("native");
            }
            let lines = native_lines(&mut app, &mut backend, record.clone(), live);
            let raw = lines
                .iter()
                .find_map(|line| {
                    (line["params"]["item"]["type"] == "reasoning").then(|| {
                        line["params"]["item"]["id"]
                            .as_str()
                            .expect("cursor")
                            .to_string()
                    })
                })
                .expect("reasoning cursor");
            let tool = json!({"type":"assistant","session_id":"thread","message":{"id":"tool-native","role":"assistant","content":[{"type":"tool_use","id":raw,"name":"Bash","input":{"command":"pwd"}}]}});
            native_lines(&mut app, &mut backend, tool, live);
            assert_eq!(app.session.messages().len(), 2);
            assert_eq!(app.session.messages()[0].role, Role::Reasoning);
            let tool = app.session.messages()[1].id;
            native_lines(&mut app, &mut backend, record, live);
            assert_eq!(
                app.session
                    .messages()
                    .entry(tool)
                    .expect("native tool survives")
                    .role,
                Role::ToolCall
            );
            app.assert_entry_invariants();
        }
    }
}

#[test]
fn a_tool_result_and_a_call_with_the_same_printable_id_have_distinct_native_rows() {
    for live in [false, true] {
        let mut app = AppState::new("thread".into());
        app.session.set_claude_enabled_for_test(true);
        app.session.set_claude_generation_for_test(1);
        let mut backend = carlos_core::claude_backend::FixtureNormalizer::default();
        let call = json!({"type":"assistant","session_id":"thread","message":{"id":"native","role":"assistant","content":[{"type":"tool_use","id":"call:result","name":"Bash","input":{"command":"pwd"}}]}});
        native_lines(&mut app, &mut backend, call, live);
        let result = json!({"type":"user","session_id":"thread","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"call","content":"done"}]}});
        native_lines(&mut app, &mut backend, result, live);
        assert_eq!(app.session.messages().len(), 2);
        assert_eq!(app.session.messages()[0].role, Role::ToolCall);
        assert_eq!(app.session.messages()[1].role, Role::ToolOutput);
        app.assert_entry_invariants();
    }
}
