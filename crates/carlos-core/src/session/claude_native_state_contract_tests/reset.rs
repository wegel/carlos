//! Preserve native pipelined prompts, reset epochs and exact callback retirement order.

use serde_json::{json, Value};

use super::{frame, init, result, sdk, state, submitted};
use crate::claude_backend::FixtureNormalizer;
use crate::session::approval_state::ApprovalQueue;
use crate::session::claude::{ClaudePhase, NativeState};
use crate::session::models::Role;
use crate::session::state::SessionState;

#[test]
fn reset_keeps_accepted_pipelined_human_and_real_child_callback_until_native_consumes_them() {
    let mut app = SessionState::new("old".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "older", "old prompt");
    init(&mut app, &mut native, "old", json!(["older"]), 0);
    result(&mut app, &mut native, "older", 0);
    state(&mut app, &mut native, "idle", 0);
    submitted(&mut app, &mut native, "clear", "/clear");
    submitted(&mut app, &mut native, "next", "Après clear 日本語");
    frame(
        &mut app,
        &mut native,
        json!({"type":"callback","kind":"permission","callbackId":"child-live","toolName":"Bash","input":{"command":"echo child"},"options":{"agentId":"child"}}),
        0,
        None,
        Value::Null,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"conversation_reset","session_id":"old","uuid":"reset","new_conversation_id":"opaque","user_message_uuid":"clear"}),
        1,
    );
    assert_eq!(app.claude().phase() as u8, ClaudePhase::Starting as u8);
    assert!(app.approval().pending().is_some());
    assert!(app.messages().is_empty());
    init(&mut app, &mut native, "actual-new", json!(["clear"]), 1);
    result(&mut app, &mut native, "clear", 1);
    init(&mut app, &mut native, "actual-new", json!(["next"]), 1);
    result(&mut app, &mut native, "next", 1);
    state(&mut app, &mut native, "idle", 1);
    assert_eq!(app.thread_id(), "actual-new");
    assert!(app.active_turn_id().is_none());
    assert_eq!(
        app.messages()
            .iter()
            .filter(|message| message.role == Role::User && message.text == "Après clear 日本語")
            .count(),
        1
    );
    assert_eq!(app.input_history().history_len(), 1);
    assert!(!app.turn_status_by_id().contains_key("clear"));
    assert!(app.approval().pending().is_some());
}

#[test]
fn old_main_events_and_context_replies_cannot_cross_a_reset_epoch_but_process_task_levels_can() {
    let mut app = SessionState::new("old".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    sdk(
        &mut app,
        &mut native,
        json!({"type":"conversation_reset","session_id":"old","uuid":"reset","new_conversation_id":"opaque"}),
        1,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"background_tasks_changed","session_id":"old","tasks":[{"task_id":"child","run_id":"z","description":"survives"}]}),
        1,
    );
    init(&mut app, &mut native, "new", json!([]), 1);
    sdk(
        &mut app,
        &mut native,
        json!({"type":"system","subtype":"session_state_changed","state":"running"}),
        0,
    );
    frame(
        &mut app,
        &mut native,
        json!({"type":"reply","ok":true,"result":{"totalTokens":99,"maxTokens":100}}),
        0,
        Some("context"),
        json!({"revision":0}),
    );
    assert!(app.context_usage().is_none());
    assert!(app.claude().activity().native_state() == NativeState::Idle);
    assert_eq!(app.claude().tasks().active_count(), 1);
    state(&mut app, &mut native, "running", 1);
    frame(
        &mut app,
        &mut native,
        json!({"type":"reply","ok":true,"result":{"totalTokens":80,"maxTokens":100}}),
        1,
        Some("context"),
        json!({"revision":0}),
    );
    assert!(app.context_usage().is_none());
}

#[test]
fn unowned_native_task_wakeup_never_consumes_the_next_human_or_runs_ralph_completion() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "human", "queued human");
    init(&mut app, &mut native, "session", json!([]), 0);
    sdk(
        &mut app,
        &mut native,
        json!({"type":"assistant","message":{"id":"native-wakeup","role":"assistant","content":[{"type":"text","text":"Child reported"}]}}),
        0,
    );
    sdk(
        &mut app,
        &mut native,
        json!({"type":"result","subtype":"success","is_error":false,"origin":{"kind":"task-notification","runId":"run"}}),
        0,
    );
    state(&mut app, &mut native, "idle", 0);
    assert!(native.begin_interrupt().is_empty());
    assert!(app.turn_status_by_id().is_empty());
    assert!(!app
        .claude_prompts()
        .find(|(id, _)| *id == "human")
        .expect("accepted human")
        .1
        .consumed());
    assert!(app.active_turn_id().is_none());
    init(&mut app, &mut native, "session", json!(["human"]), 0);
    result(&mut app, &mut native, "human", 0);
    state(&mut app, &mut native, "idle", 0);
    assert!(app.claude_prompts().next().is_none());
    assert_eq!(app.turn_status_by_id().len(), 1);
}

#[test]
fn reset_keeps_every_active_queued_callback_draft_and_confirmation_until_its_exact_native_cancel() {
    let mut app = SessionState::new("old".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    for id in ["active", "queued-one", "queued-two"] {
        frame(
            &mut app,
            &mut native,
            json!({"type":"callback","kind":"permission","callbackId":id,"payload":{"toolName":"Bash","input":{"command":"echo"}}}),
            0,
            None,
            Value::Null,
        );
    }
    sdk(
        &mut app,
        &mut native,
        json!({"type":"conversation_reset","session_id":"old","uuid":"reset","new_conversation_id":"opaque"}),
        1,
    );
    assert_eq!(
        app.approval()
            .queued(ApprovalQueue::Claude)
            .iter()
            .map(|request| request.facts().request_id.as_str().expect("id"))
            .collect::<Vec<_>>(),
        ["queued-one", "queued-two"]
    );
    assert_eq!(app.export_approvals().len(), 3);
    frame(
        &mut app,
        &mut native,
        json!({"type":"event","event":"callback_cancelled","callbackId":"queued-one"}),
        0,
        None,
        Value::Null,
    );
    assert_eq!(
        app.approval().pending().expect("active").facts().request_id,
        "active"
    );
    let _ = app.clear_pending_session_approval();
    assert_eq!(
        app.approval()
            .pending()
            .expect("remaining")
            .facts()
            .request_id,
        "queued-two"
    );
    assert_eq!(app.export_approvals().len(), 1);
}
