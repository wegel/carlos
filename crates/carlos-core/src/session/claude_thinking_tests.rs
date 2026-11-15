//! Apply native thinking streams and snapshots to Core without copying hidden signatures into rows.

use serde_json::{json, Value};

use super::backend_test_support::{action, context};
use super::models::Role;
use super::state::SessionState;
use crate::claude_backend::{translate_claude_line, ClaudeTranslationState};

fn translate(core: &mut SessionState, state: &mut ClaudeTranslationState, message: Value) {
    for line in translate_claude_line(state, &message.to_string())
        .expect("native translation")
        .lines
    {
        let update = core.handle_backend_line(&line, &context(core));
        assert!(
            action(update).is_none(),
            "fixed streamed items need no native read"
        );
    }
}

#[test]
fn streamed_thinking_and_native_per_block_snapshots_do_not_repeat_or_hide_text() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
    let mut state = ClaudeTranslationState::default();
    for event in [
        json!({"type":"message_start","message":{"id":"api-message"}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"thinking","thinking":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Visible thought"}}),
        json!({"type":"content_block_stop","index":0}),
    ] {
        translate(
            &mut core,
            &mut state,
            json!({"type":"stream_event","event":event}),
        );
    }
    translate(
        &mut core,
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
            &mut core,
            &mut state,
            json!({"type":"stream_event","event":event}),
        );
    }
    translate(
        &mut core,
        &mut state,
        json!({"type":"assistant","uuid":"answer-uuid","message":{"id":"api-message",
        "role":"assistant","content":[{"type":"text","text":"Initial answer"}]}}),
    );
    assert_eq!(count_role(&core, Role::Reasoning), 1);
    assert_eq!(count_role(&core, Role::Assistant), 1);
    assert_eq!(
        core.messages()
            .iter()
            .find(|message| message.role == Role::Assistant)
            .expect("answer")
            .text,
        "Initial answer"
    );
    assert!(core
        .messages()
        .iter()
        .all(|message| !message.text.contains("DO_NOT_RENDER")));
}

#[test]
fn snapshot_only_blocks_with_one_api_id_keep_thinking_and_answer_and_ignore_replays() {
    let mut core = SessionState::new("session".into(), Default::default(), false);
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
            &mut core,
            &mut state,
            json!({"type":"assistant","uuid":id,"message":{"id":"same-api-id",
            "role":"assistant","content":[part]}}),
        );
    }
    assert_eq!(core.messages().len(), 2);
    assert_eq!(core.messages()[0].role, Role::Reasoning);
    assert_eq!(core.messages()[0].text, "Snapshot thought");
    assert_eq!(core.messages()[1].text, "Snapshot answer");
    assert!(core
        .messages()
        .iter()
        .all(|message| !message.text.contains("HIDDEN_PAYLOAD")));
}

fn count_role(core: &SessionState, role: Role) -> usize {
    core.messages()
        .iter()
        .filter(|message| message.role == role)
        .count()
}
