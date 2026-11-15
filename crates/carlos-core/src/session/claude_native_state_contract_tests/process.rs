//! Keep hidden children, compaction counts and recoverable notices at the native reducer.

use serde_json::json;

use super::{frame, sdk};
use crate::claude_backend::FixtureNormalizer;
use crate::session::claude::ClaudePhase;
use crate::session::state::SessionState;

#[test]
fn late_child_text_stays_hidden_and_thinking_estimates_never_become_text_or_usage() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"thinking_tokens","estimated_tokens":43}),
        0,
    );
    assert_eq!(app.claude().activity().thinking(), Some(43));
    assert!(app.context_usage().is_none());
    assert!(app.messages().is_empty());
    sdk(
        &mut app,
        &mut native,
        json!({"type":"assistant","agent_id":"child","parent_tool_use_id":"caller","uuid":"child-text","message":{"id":"child-message","role":"assistant","content":[{"type":"text","text":"Late child 日本語"}]}}),
        0,
    );
    assert!(app.messages().is_empty());
    assert!(app.active_turn_id().is_none());
}

#[test]
fn compact_status_and_boundary_share_one_notice_and_failed_attempt_allows_fresh_context() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"status","status":"compacting"}),
        0,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"status","status":null,"compact_result":"failed","compact_error":"fixture error"}),
        0,
    );
    assert_eq!(app.messages().len(), 1);
    assert!(app.messages()[0].text.contains("fixture error"));
    assert!(app.claude().activity().transient().is_none());
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"status","status":"compacting"}),
        0,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"compact_boundary","uuid":"boundary"}),
        0,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"status","status":null,"compact_result":"success"}),
        0,
    );
    assert_eq!(app.messages().len(), 2);
    assert_eq!(app.messages()[1].text, "Claude compacted context");
    let revision = app.claude().activity().context_revision();
    frame(
        &mut app,
        &mut native,
        json!({"type":"reply","ok":true,"result":{"totalTokens":20,"maxTokens":100}}),
        0,
        Some("context"),
        json!({"revision":revision}),
    );
    assert_eq!(app.context_usage().expect("fresh native context").used, 20);
}

#[test]
fn recoverable_native_notices_do_not_close_a_healthy_query() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"init","plugin_errors":[{"plugin":"bad plugin","message":"invalid setting","path":"plugin.json"}]}),
        0,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"session_writes_lost","uuid":"lost","error_code":"ENOSPC","lost":["transcript"]}),
        0,
    );
    assert!(app
        .messages()
        .iter()
        .any(|message| message.text.contains("invalid setting")));
    assert!(app
        .messages()
        .iter()
        .any(|message| message.text.contains("ENOSPC")));
    assert!(app.claude().phase() == ClaudePhase::Ready);
}

#[test]
fn many_hidden_children_keep_the_session_healthy_and_task_state_available() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    for index in 0..180 {
        let child = format!("child-{index}");
        sdk(
            &mut app,
            &mut native,
            json!({"type":"assistant","agent_id":child,"parent_tool_use_id":child,"uuid":format!("text-{index}"),"message":{"id":format!("api-{index}"),"role":"assistant","content":[{"type":"text","text":format!("Child output {index}")}]}}),
            0,
        );
        sdk(
            &mut app,
            &mut native,
            json!({"type":"system","subtype":"task_notification","task_id":child,"tool_use_id":child,"run_id":"a","status":"completed","summary":"done"}),
            0,
        );
    }
    assert!(app.claude().phase() == ClaudePhase::Ready);
    assert!(app.messages().is_empty());
    assert!(app.claude().tasks().data("child-179").is_some());
}

#[test]
fn older_child_messages_without_agent_ids_stay_hidden_without_cursor_allocations() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    for index in 0..140 {
        let child = format!("child-{index}");
        let caller = format!("caller-{index}");
        sdk(
            &mut app,
            &mut native,
            json!({"type":"system","subtype":"task_started","task_id":child,"run_id":"a","tool_use_id":caller,"description":"child"}),
            0,
        );
        sdk(
            &mut app,
            &mut native,
            json!({"type":"assistant","parent_tool_use_id":caller,"uuid":format!("text-{index}"),"message":{"id":format!("api-{index}"),"role":"assistant","content":[{"type":"text","text":format!("Legacy child output {index}")}]}}),
            0,
        );
        sdk(
            &mut app,
            &mut native,
            json!({"type":"system","subtype":"task_notification","task_id":child,"run_id":"a","status":"completed","summary":"done"}),
            0,
        );
    }
    assert!(app.claude().phase() == ClaudePhase::Ready);
    assert!(app.messages().is_empty());
    assert!(app.claude().tasks().data("child-139").is_some());
}
