//! Check native thinking streams, per-block snapshots, and redacted payload handling.

use serde_json::{json, Value};

use crate::claude_backend::{translate_claude_line, ClaudeTranslationState};

use super::models::Role;
use super::notifications::handle_server_message_line;
use super::state::AppState;

fn translate(app: &mut AppState, state: &mut ClaudeTranslationState, message: Value) {
    for line in translate_claude_line(state, &message.to_string())
        .expect("native translation")
        .lines
    {
        let _ = handle_server_message_line(app, &line);
    }
}

#[test]
fn streamed_thinking_and_native_per_block_snapshots_do_not_repeat_or_hide_text() {
    let mut app = AppState::new("session".to_string());
    let mut state = ClaudeTranslationState::default();
    for event in [
        json!({"type":"message_start","message":{"id":"api-message"}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Visible thought"}}),
        json!({"type":"content_block_stop","index":0}),
    ] {
        translate(
            &mut app,
            &mut state,
            json!({"type":"stream_event","event":event}),
        );
    }
    translate(
        &mut app,
        &mut state,
        json!({"type":"assistant","uuid":"thinking-uuid","message":{"id":"api-message",
        "role":"assistant","content":[{"type":"thinking","thinking":"Visible thought","signature":"DO_NOT_RENDER"}]}}),
    );
    for event in [
        json!({"type":"content_block_start","index":1,"content_block":{"type":"text","text":"Initial "}}),
        json!({"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"answer"}}),
        json!({"type":"content_block_stop","index":1}),
    ] {
        translate(
            &mut app,
            &mut state,
            json!({"type":"stream_event","event":event}),
        );
    }
    translate(
        &mut app,
        &mut state,
        json!({"type":"assistant","uuid":"answer-uuid","message":{"id":"api-message",
        "role":"assistant","content":[{"type":"text","text":"Initial answer"}]}}),
    );
    assert_eq!(count_role(&app, Role::Reasoning), 1);
    assert_eq!(count_role(&app, Role::Assistant), 1);
    assert_eq!(
        app.messages
            .iter()
            .find(|message| message.role == Role::Assistant)
            .expect("answer")
            .text,
        "Initial answer"
    );
    assert!(app
        .messages
        .iter()
        .all(|message| !message.text.contains("DO_NOT_RENDER")));
}

#[test]
fn snapshot_only_blocks_with_one_api_id_keep_thinking_and_answer_and_ignore_replays() {
    let mut app = AppState::new("session".to_string());
    let mut state = ClaudeTranslationState::default();
    for (id, part) in [
        (
            "one",
            json!({"type":"thinking","thinking":"Snapshot thought"}),
        ),
        ("two", json!({"type":"text","text":"Snapshot answer"})),
        ("two", json!({"type":"text","text":"Snapshot answer"})),
        (
            "three",
            json!({"type":"redacted_thinking","data":"HIDDEN_PAYLOAD"}),
        ),
    ] {
        translate(
            &mut app,
            &mut state,
            json!({"type":"assistant","uuid":id,"message":{"id":"same-api-id",
            "role":"assistant","content":[part]}}),
        );
    }
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[0].role, Role::Reasoning);
    assert_eq!(app.messages[0].text, "Snapshot thought");
    assert_eq!(app.messages[1].text, "Snapshot answer");
    assert!(app
        .messages
        .iter()
        .all(|message| !message.text.contains("HIDDEN_PAYLOAD")));
}

fn count_role(app: &AppState, role: Role) -> usize {
    app.messages
        .iter()
        .filter(|message| message.role == role)
        .count()
}
