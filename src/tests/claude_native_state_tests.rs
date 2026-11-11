//! Native reset, process activity, late child output, and context-read ordering at the shared UI entrypoint.

use serde_json::{json, Value};

use super::claude_activity::NativeState;
use super::claude_state::ClaudePhase;
use super::models::Role;
use super::notifications::handle_server_message_line;
use super::state::AppState;

fn frame(app: &mut AppState, frame: Value, epoch: u64, method: Option<&str>, context: Value) {
    let _ = handle_server_message_line(
        app,
        &json!({"method":"claude/controller","params":{
        "frame":frame,"epoch":epoch,"requestMethod":method,"requestContext":context}})
        .to_string(),
    );
}
fn sdk(app: &mut AppState, message: Value, epoch: u64) {
    frame(
        app,
        json!({"v":1,"type":"event","event":"sdk","message":message}),
        epoch,
        None,
        Value::Null,
    );
}
fn submitted(app: &mut AppState, id: &str, text: &str) {
    let index = app.append_message(Role::User, text);
    app.record_input_history(text, Some(index));
    super::claude_prompts::attach_echo(app, id);
    let _ = handle_server_message_line(app,&json!({"method":"claude/promptSubmitted","params":{"uuid":id,"text":text,"origin":"human","priority":"later"}}).to_string());
}
fn init(app: &mut AppState, id: &str, prompts: Value, epoch: u64) {
    sdk(
        app,
        json!({"type":"system","subtype":"init","session_id":id,"user_message_uuids":prompts}),
        epoch,
    );
}
fn state(app: &mut AppState, name: &str, epoch: u64) {
    sdk(
        app,
        json!({"type":"system","subtype":"session_state_changed","state":name}),
        epoch,
    );
}
fn result(app: &mut AppState, id: &str, epoch: u64) {
    sdk(
        app,
        json!({"type":"result","subtype":"success","is_error":false,"user_message_uuid":id}),
        epoch,
    );
}

#[test]
fn reset_keeps_accepted_pipelined_human_and_real_child_callback_until_native_consumes_them() {
    let mut app = AppState::new("old".to_string());
    submitted(&mut app, "older", "old prompt");
    init(&mut app, "old", json!(["older"]), 0);
    result(&mut app, "older", 0);
    state(&mut app, "idle", 0);
    submitted(&mut app, "clear", "/clear");
    submitted(&mut app, "next", "Après clear 日本語");
    frame(
        &mut app,
        json!({"type":"callback","kind":"permission","callbackId":"child-live","toolName":"Bash","input":{"command":"echo child"},"options":{"agentId":"child"}}),
        0,
        None,
        Value::Null,
    );
    app.set_input_text("new unsent draft");
    sdk(
        &mut app,
        json!({"type":"conversation_reset","session_id":"old","uuid":"reset","new_conversation_id":"opaque","user_message_uuid":"clear"}),
        1,
    );
    assert_eq!(app.claude.phase as u8, ClaudePhase::Starting as u8);
    assert!(app.approval.pending.is_some());
    assert!(app.messages.is_empty());
    assert_eq!(app.input_text(), "new unsent draft");
    init(&mut app, "actual-new", json!(["clear"]), 1);
    result(&mut app, "clear", 1);
    init(&mut app, "actual-new", json!(["next"]), 1);
    result(&mut app, "next", 1);
    state(&mut app, "idle", 1);
    assert_eq!(app.thread_id, "actual-new");
    assert!(app.active_turn_id.is_none());
    assert_eq!(
        app.messages
            .iter()
            .filter(|message| message.role == Role::User && message.text == "Après clear 日本語")
            .count(),
        1
    );
    assert_eq!(app.submitted_turn_count(), 1);
    assert!(!app.turn_status_by_id.contains_key("clear"));
    assert!(app.approval.pending.is_some());
}

#[test]
fn old_main_events_and_context_replies_cannot_cross_a_reset_epoch_but_process_task_levels_can() {
    let mut app = AppState::new("old".to_string());
    sdk(
        &mut app,
        json!({"type":"conversation_reset","session_id":"old","uuid":"reset","new_conversation_id":"opaque"}),
        1,
    );
    sdk(
        &mut app,
        json!({"type":"system","subtype":"background_tasks_changed","session_id":"old","tasks":[{"task_id":"child","run_id":"z","description":"survives"}]}),
        1,
    );
    init(&mut app, "new", json!([]), 1);
    sdk(
        &mut app,
        json!({"type":"system","subtype":"session_state_changed","state":"running"}),
        0,
    );
    frame(
        &mut app,
        json!({"type":"reply","ok":true,"result":{"totalTokens":99,"maxTokens":100}}),
        0,
        Some("context"),
        json!({"revision":0}),
    );
    assert!(app.context_usage.is_none());
    assert!(app.claude.activity.state == NativeState::Idle);
    assert_eq!(app.claude.tasks.active_count(), 1);
    state(&mut app, "running", 1);
    frame(
        &mut app,
        json!({"type":"reply","ok":true,"result":{"totalTokens":80,"maxTokens":100}}),
        1,
        Some("context"),
        json!({"revision":0}),
    );
    assert!(app.context_usage.is_none());
}

#[test]
fn unowned_native_task_wakeup_never_consumes_the_next_human_or_runs_ralph_completion() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "human", "queued human");
    init(&mut app, "session", json!([]), 0);
    sdk(
        &mut app,
        json!({"type":"assistant","message":{"id":"native-wakeup","role":"assistant","content":[{"type":"text","text":"Child reported"}]}}),
        0,
    );
    sdk(
        &mut app,
        json!({"type":"result","subtype":"success","is_error":false,"origin":{"kind":"task-notification","runId":"run"}}),
        0,
    );
    state(&mut app, "idle", 0);
    assert!(app.claude.begin_interrupt().is_empty());
    assert!(app.turn_status_by_id.is_empty());
    assert!(!app.claude.prompts.entries["human"].consumed);
    assert!(app.active_turn_id.is_none());
    init(&mut app, "session", json!(["human"]), 0);
    result(&mut app, "human", 0);
    state(&mut app, "idle", 0);
    assert!(app.claude.prompts.entries.is_empty());
    assert_eq!(app.turn_status_by_id.len(), 1);
}

#[test]
fn late_child_text_stays_hidden_and_thinking_estimates_never_become_text_or_usage() {
    let mut app = AppState::new("session".to_string());
    sdk(
        &mut app,
        json!({"type":"system","subtype":"thinking_tokens","estimated_tokens":43}),
        0,
    );
    assert_eq!(app.claude.activity.thinking, Some(43));
    assert!(app.context_usage.is_none());
    assert!(app.messages.is_empty());
    sdk(
        &mut app,
        json!({"type":"assistant","agent_id":"child","parent_tool_use_id":"caller","uuid":"child-text","message":{"id":"child-message","role":"assistant","content":[{"type":"text","text":"Late child 日本語"}]}}),
        0,
    );
    assert!(app.messages.is_empty());
    assert!(app.active_turn_id.is_none());
}

#[test]
fn compact_status_and_boundary_share_one_notice_and_failed_attempt_allows_fresh_context() {
    let mut app = AppState::new("session".to_string());
    sdk(
        &mut app,
        json!({"type":"system","subtype":"status","status":"compacting"}),
        0,
    );
    sdk(
        &mut app,
        json!({"type":"system","subtype":"status","status":null,"compact_result":"failed","compact_error":"fixture error"}),
        0,
    );
    assert_eq!(app.messages.len(), 1);
    assert!(app.messages[0].text.contains("fixture error"));
    assert!(app.claude.activity.transient.is_none());
    sdk(
        &mut app,
        json!({"type":"system","subtype":"status","status":"compacting"}),
        0,
    );
    sdk(
        &mut app,
        json!({"type":"system","subtype":"compact_boundary","uuid":"boundary"}),
        0,
    );
    sdk(
        &mut app,
        json!({"type":"system","subtype":"status","status":null,"compact_result":"success"}),
        0,
    );
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.messages[1].text, "Claude compacted context");
    let revision = app.claude.activity.context_revision;
    frame(
        &mut app,
        json!({"type":"reply","ok":true,"result":{"totalTokens":20,"maxTokens":100}}),
        0,
        Some("context"),
        json!({"revision":revision}),
    );
    assert_eq!(app.context_usage.expect("fresh native context").used, 20);
}

#[test]
fn recoverable_native_notices_do_not_close_a_healthy_query() {
    let mut app = AppState::new("session".to_string());
    sdk(
        &mut app,
        json!({"type":"system","subtype":"init","plugin_errors":[{"plugin":"bad plugin","message":"invalid setting","path":"plugin.json"}]}),
        0,
    );
    sdk(
        &mut app,
        json!({"type":"system","subtype":"session_writes_lost","uuid":"lost","error_code":"ENOSPC","lost":["transcript"]}),
        0,
    );
    assert!(app
        .messages
        .iter()
        .any(|message| message.text.contains("invalid setting")));
    assert!(app
        .messages
        .iter()
        .any(|message| message.text.contains("ENOSPC")));
    assert!(app.claude.phase == ClaudePhase::Ready);
}

#[test]
fn reset_keeps_every_active_queued_callback_draft_and_confirmation_until_its_exact_native_cancel() {
    let mut app = AppState::new("old".to_string());
    for id in ["active", "queued-one", "queued-two"] {
        frame(
            &mut app,
            json!({"type":"callback","kind":"permission","callbackId":id,"payload":{"toolName":"Bash","input":{"command":"echo"}}}),
            0,
            None,
            Value::Null,
        );
    }
    app.claude.confirm_allow = Some("active".to_string());
    sdk(
        &mut app,
        json!({"type":"conversation_reset","session_id":"old","uuid":"reset","new_conversation_id":"opaque"}),
        1,
    );
    assert_eq!(
        app.claude
            .questions
            .iter()
            .map(|request| request.request_id.as_str().expect("id"))
            .collect::<Vec<_>>(),
        ["queued-one", "queued-two"]
    );
    assert_eq!(app.claude.answers.drafts.len(), 3);
    assert_eq!(app.claude.confirm_allow.as_deref(), Some("active"));
    frame(
        &mut app,
        json!({"type":"event","event":"callback_cancelled","callbackId":"queued-one"}),
        0,
        None,
        Value::Null,
    );
    assert_eq!(
        app.approval.pending.as_ref().expect("active").request_id,
        "active"
    );
    app.clear_pending_approval();
    assert_eq!(
        app.approval.pending.as_ref().expect("remaining").request_id,
        "queued-two"
    );
    assert_eq!(app.claude.answers.drafts.len(), 1);
}

#[test]
fn many_hidden_children_keep_the_session_healthy_and_task_state_available() {
    let mut app = AppState::new("session".to_string());
    for index in 0..180 {
        let child = format!("child-{index}");
        sdk(
            &mut app,
            json!({"type":"assistant","agent_id":child,"parent_tool_use_id":child,"uuid":format!("text-{index}"),"message":{"id":format!("api-{index}"),"role":"assistant","content":[{"type":"text","text":format!("Child output {index}")}]}}),
            0,
        );
        sdk(
            &mut app,
            json!({"type":"system","subtype":"task_notification","task_id":child,"tool_use_id":child,"run_id":"a","status":"completed","summary":"done"}),
            0,
        );
    }
    assert!(app.claude.phase == ClaudePhase::Ready);
    assert!(app.messages.is_empty());
    assert!(app.claude.tasks.data("child-179").is_some());
}

#[test]
fn older_child_messages_without_agent_ids_stay_hidden_without_cursor_allocations() {
    let mut app = AppState::new("session".to_string());
    for index in 0..140 {
        let child = format!("child-{index}");
        let caller = format!("caller-{index}");
        sdk(
            &mut app,
            json!({"type":"system","subtype":"task_started","task_id":child,"run_id":"a","tool_use_id":caller,"description":"child"}),
            0,
        );
        sdk(
            &mut app,
            json!({"type":"assistant","parent_tool_use_id":caller,"uuid":format!("text-{index}"),"message":{"id":format!("api-{index}"),"role":"assistant","content":[{"type":"text","text":format!("Legacy child output {index}")}]}}),
            0,
        );
        sdk(
            &mut app,
            json!({"type":"system","subtype":"task_notification","task_id":child,"run_id":"a","status":"completed","summary":"done"}),
            0,
        );
    }
    assert!(app.claude.phase == ClaudePhase::Ready);
    assert!(app.messages.is_empty());
    assert!(app.claude.tasks.data("child-139").is_some());
}
