//! Native SDK prompt, result, question, and context checks at the Rust UI boundary.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::{json, Value};

use super::models::TurnStatus;
use super::notifications::handle_server_message_line;
use super::ralph::RalphConfig;
use super::state::AppState;
use crate::backend::PromptOrigin;

fn frame(app: &mut AppState, frame: Value, method: Option<&str>) {
    let _ = handle_server_message_line(
        app,
        &json!({ "method": "claude/controller",
        "params": { "frame": frame, "requestMethod": method } })
        .to_string(),
    );
}

fn sdk(app: &mut AppState, message: Value) {
    frame(
        app,
        json!({ "v": 1, "type": "event", "event": "sdk", "message": message }),
        None,
    );
}

fn submitted(app: &mut AppState, id: &str) {
    app.record_turn_started(id);
    let _ = handle_server_message_line(
        app,
        &json!({ "method": "claude/promptSubmitted", "params": { "uuid": id } }).to_string(),
    );
}

fn idle(app: &mut AppState) {
    sdk(
        app,
        json!({ "type": "system", "subtype": "session_state_changed", "state": "idle" }),
    );
}

fn result(id: &str) -> Value {
    json!({ "type": "result", "subtype": "success", "is_error": false, "user_message_uuid": id })
}

#[test]
fn result_waits_for_idle_before_releasing_the_active_prompt() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "first");
    sdk(&mut app, result("first"));
    assert_eq!(app.active_turn_id.as_deref(), Some("first"));
    idle(&mut app);
    assert!(app.active_turn_id.is_none());
    assert_eq!(
        app.turn_status_by_id.get("first"),
        Some(&TurnStatus::Completed)
    );
}

#[test]
fn idle_before_result_completes_only_after_the_result_arrives() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "first");
    idle(&mut app);
    assert_eq!(app.active_turn_id.as_deref(), Some("first"));
    sdk(&mut app, result("first"));
    assert!(app.active_turn_id.is_none());
}

#[test]
fn previous_steer_cycle_cannot_complete_a_newer_prompt() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "first");
    sdk(
        &mut app,
        json!({ "type": "assistant", "user_message_uuid": "first", "message": { "content": [] } }),
    );
    submitted(&mut app, "steer");
    sdk(&mut app, result("first"));
    idle(&mut app);
    assert_eq!(app.active_turn_id.as_deref(), Some("steer"));
    sdk(
        &mut app,
        json!({ "type": "assistant", "user_message_uuid": "steer", "message": { "content": [] } }),
    );
    sdk(&mut app, result("steer"));
    assert_eq!(app.active_turn_id.as_deref(), Some("steer"));
    idle(&mut app);
    assert!(app.active_turn_id.is_none());
}

#[test]
fn a_coalesced_native_result_completes_every_consumed_prompt_uuid() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "first");
    submitted(&mut app, "second");
    let mut message = result("second");
    message["user_message_uuids"] = json!(["first", "second"]);
    sdk(&mut app, message);
    idle(&mut app);
    assert!(app.active_turn_id.is_none());
    for id in ["first", "second"] {
        assert_eq!(app.turn_status_by_id.get(id), Some(&TurnStatus::Completed));
    }
}

#[test]
fn sdk_error_is_failed_even_when_an_interrupt_was_requested() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "first");
    sdk(
        &mut app,
        json!({ "type": "assistant", "user_message_uuid": "first", "message": { "content": [] } }),
    );
    app.claude.begin_interrupt();
    let mut error = result("first");
    error["is_error"] = json!(true);
    error["errors"] = json!(["Model failed"]);
    sdk(&mut app, error);
    idle(&mut app);
    assert_eq!(
        app.turn_status_by_id.get("first"),
        Some(&TurnStatus::Failed)
    );
}

#[test]
fn empty_sdk_errors_fall_back_to_native_result_text_then_a_useful_generic_error() {
    for (text, expected) in [
        ("Native failure detail", "Native failure detail"),
        (" \t\n", "Claude turn failed"),
    ] {
        let mut app = AppState::new("session".to_string());
        app.enable_ralph_mode(RalphConfig {
            prompt_path: PathBuf::from("fixture"),
            base_prompt: "start".to_string(),
            continuation_prompt: "continue".to_string(),
            done_marker: "@@DONE@@".to_string(),
            blocked_marker: "@@BLOCKED@@".to_string(),
            waiting_marker: "@@WAITING@@".to_string(),
        });
        submitted(&mut app, "first");
        let mut error = result("first");
        error["is_error"] = json!(true);
        error["errors"] = json!([]);
        error["result"] = json!(text);
        sdk(&mut app, error);
        idle(&mut app);
        assert_eq!(app.messages.last().expect("failure message").text, expected);
        assert_eq!(
            app.turn_status_by_id.get("first"),
            Some(&TurnStatus::Failed)
        );
        assert!(!app.ralph_enabled());
    }
}

#[test]
fn native_abort_reasons_remain_interrupts_even_if_the_result_has_an_error_flag() {
    for reason in ["aborted_streaming", "aborted_tools"] {
        let mut app = AppState::new("session".to_string());
        submitted(&mut app, "first");
        let mut message = result("first");
        message["terminal_reason"] = json!(reason);
        message["is_error"] = json!(true);
        sdk(&mut app, message);
        idle(&mut app);
        assert_eq!(
            app.turn_status_by_id.get("first"),
            Some(&TurnStatus::Interrupted)
        );
    }
}

#[test]
fn failed_interrupt_rolls_back_only_the_captured_prompt_ids() {
    let mut app = AppState::new("session".to_string());
    submitted(&mut app, "first");
    sdk(
        &mut app,
        json!({"type":"assistant","user_message_uuid":"first","message":{"role":"assistant","content":[]}}),
    );
    let captured = app.claude.begin_interrupt();
    assert_eq!(captured, ["first"]);
    app.claude.rollback_interrupt(&captured);
    sdk(&mut app, result("first"));
    idle(&mut app);
    assert_eq!(
        app.turn_status_by_id.get("first"),
        Some(&TurnStatus::Completed)
    );
}

#[test]
fn failed_ralph_turn_discards_automatic_work_and_preserves_queued_human_input() {
    let mut app = AppState::new("session".to_string());
    app.enable_ralph_mode(RalphConfig {
        prompt_path: PathBuf::from("fixture"),
        base_prompt: "start".to_string(),
        continuation_prompt: "continue".to_string(),
        done_marker: "@@DONE@@".to_string(),
        blocked_marker: "@@BLOCKED@@".to_string(),
        waiting_marker: "@@WAITING@@".to_string(),
    });
    app.queue_turn_input("human next");
    app.queue_ralph_continuation("automatic next");
    submitted(&mut app, "first");
    let mut error = result("first");
    error["is_error"] = json!(true);
    sdk(&mut app, error);
    idle(&mut app);
    assert!(!app.ralph_enabled());
    let queued = app
        .dequeue_turn_input(Instant::now())
        .expect("human input retained");
    assert_eq!(queued.text, "human next");
    assert_eq!(queued.origin, PromptOrigin::Human);
    assert!(app.dequeue_turn_input(Instant::now()).is_none());
}

#[test]
fn startup_callback_remains_visible_while_the_sdk_initializes() {
    let mut app = AppState::new("session".to_string());
    app.claude.phase = crate::app::claude_state::ClaudePhase::Starting;
    frame(
        &mut app,
        json!({ "v": 1, "type": "callback", "kind": "permission", "callbackId": "startup",
        "payload": { "title": "Allow startup?", "toolName": "Bash", "input": { "command": "pwd" } } }),
        None,
    );
    assert_eq!(
        app.approval
            .pending
            .as_ref()
            .expect("startup question")
            .title,
        "Allow startup?"
    );
    assert!(app.claude.phase != crate::app::claude_state::ClaudePhase::Ready);
    frame(
        &mut app,
        json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [] } }),
        Some("start"),
    );
    assert!(app.claude.phase == crate::app::claude_state::ClaudePhase::Ready);
    assert!(app.approval.pending.is_some());
}

#[test]
fn callback_cancellation_promotes_the_next_native_request_without_losing_it() {
    let mut app = AppState::new("session".to_string());
    for id in ["first", "second"] {
        frame(
            &mut app,
            json!({ "v": 1, "type": "callback", "kind": "permission", "callbackId": id,
            "payload": { "toolName": "Read", "input": {} } }),
            None,
        );
    }
    frame(
        &mut app,
        json!({ "v": 1, "type": "event", "event": "callback_cancelled", "callbackId": "first" }),
        None,
    );
    assert_eq!(
        app.approval
            .pending
            .as_ref()
            .expect("second request")
            .request_id,
        json!("second")
    );
}

#[test]
fn models_and_context_come_from_native_answers_without_guessing() {
    let mut app = AppState::new("session".to_string());
    frame(
        &mut app,
        json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [
        { "value": "future-model", "displayName": "Future", "supportedEffortLevels": ["high", "max"] }
    ] } }),
        Some("start"),
    );
    assert_eq!(app.runtime.available_models[0].model, "future-model");
    assert_eq!(
        app.runtime.available_models[0].supported_efforts,
        ["high", "max"]
    );
    frame(
        &mut app,
        json!({ "v": 1, "type": "reply", "ok": true,
        "result": { "totalTokens": 123, "rawMaxTokens": 456 } }),
        Some("context"),
    );
    assert_eq!(app.context_usage.expect("native context").used, 123);
    assert_eq!(app.context_usage.expect("native context").max, 456);
}

#[test]
fn initial_native_model_observation_does_not_overwrite_a_carlos_choice() {
    for selected in [None, Some("saved-model".to_string())] {
        let mut app = AppState::new("session".to_string());
        app.set_runtime_settings(selected.clone(), Some("high".to_string()), None);
        sdk(
            &mut app,
            json!({ "type": "system", "subtype": "init", "session_id": "session", "model": "native-default" }),
        );
        assert_eq!(
            app.runtime.current_model,
            selected.or_else(|| Some("native-default".to_string()))
        );
        assert_eq!(app.runtime.current_effort.as_deref(), Some("high"));
    }
}

#[test]
fn native_model_metadata_rejects_saved_other_backend_choices_in_either_startup_order() {
    for init_first in [true, false] {
        let mut app = AppState::new("session".to_string());
        app.set_runtime_settings(Some("gpt-5.5".to_string()), Some("xhigh".to_string()), None);
        let init = json!({ "type": "system", "subtype": "init", "session_id": "session", "model": "claude-native" });
        let capabilities = json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [
            { "value": "sonnet", "resolvedModel": "claude-native", "supportedEffortLevels": ["low", "high"] }
        ] } });
        if init_first {
            sdk(&mut app, init.clone());
        }
        frame(&mut app, capabilities, Some("start"));
        if !init_first {
            sdk(&mut app, init);
        }
        assert_eq!(app.runtime.current_model.as_deref(), Some("sonnet"));
        assert!(app.runtime.current_effort.is_none());
    }
}

#[test]
fn native_resolved_model_metadata_keeps_a_saved_canonical_id_selectable() {
    let mut app = AppState::new("session".to_string());
    app.set_runtime_settings(
        Some("claude-opus-current".to_string()),
        Some("max".to_string()),
        None,
    );
    frame(
        &mut app,
        json!({ "v": 1, "type": "reply", "ok": true, "result": { "models": [
        { "value": "opus", "resolvedModel": "claude-opus-current", "supportedEffortLevels": ["high", "max"] }
    ] } }),
        Some("start"),
    );
    assert_eq!(
        app.runtime.current_model.as_deref(),
        Some("claude-opus-current")
    );
    assert_eq!(app.runtime.current_effort.as_deref(), Some("max"));
    assert_eq!(app.runtime.available_models[0].model, "claude-opus-current");
}

#[test]
fn undeclared_dialog_error_does_not_disable_live_controls() {
    let mut app = AppState::new("session".to_string());
    frame(
        &mut app,
        json!({ "v": 1, "type": "event", "event": "error", "callbackId": "unsupported",
        "dialogKind": "future", "error": { "code": "unsupported_dialog", "message": "Cannot render future dialog" } }),
        None,
    );
    assert!(app.claude.phase == crate::app::claude_state::ClaudePhase::Ready);
    assert!(app.approval.pending.is_none());
    let error = &app.messages.last().expect("visible error").text;
    assert!(error.contains("Cannot render future dialog"));
    assert!(error.contains("request unsupported, kind future"));
}
