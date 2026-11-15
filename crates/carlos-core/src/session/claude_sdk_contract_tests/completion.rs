//! Preserve exact SDK result/idle, failure, abort and queued-human order in Core.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::json;

use super::{idle, result, sdk, submitted};
use crate::backend::PromptOrigin;
use crate::claude_backend::FixtureNormalizer;
use crate::session::models::TurnStatus;
use crate::session::ralph::RalphConfig;
use crate::session::state::SessionState;

#[test]
fn result_waits_for_idle_before_releasing_the_active_prompt() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "first");
    sdk(&mut app, &mut native, result("first"));
    assert_eq!(app.active_turn_id().as_deref(), Some("first"));
    idle(&mut app, &mut native);
    assert!(app.active_turn_id().is_none());
    assert_eq!(
        app.turn_status_by_id().get("first"),
        Some(&TurnStatus::Completed)
    );
}

#[test]
fn idle_before_result_completes_only_after_the_result_arrives() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "first");
    idle(&mut app, &mut native);
    assert_eq!(app.active_turn_id().as_deref(), Some("first"));
    sdk(&mut app, &mut native, result("first"));
    assert!(app.active_turn_id().is_none());
}

#[test]
fn previous_steer_cycle_cannot_complete_a_newer_prompt() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "first");
    sdk(
        &mut app,
        &mut native,
        json!({ "type": "assistant", "user_message_uuid": "first", "message": { "content": [] } }),
    );
    submitted(&mut app, &mut native, "steer");
    sdk(&mut app, &mut native, result("first"));
    idle(&mut app, &mut native);
    assert_eq!(app.active_turn_id().as_deref(), Some("steer"));
    sdk(
        &mut app,
        &mut native,
        json!({ "type": "assistant", "user_message_uuid": "steer", "message": { "content": [] } }),
    );
    sdk(&mut app, &mut native, result("steer"));
    assert_eq!(app.active_turn_id().as_deref(), Some("steer"));
    idle(&mut app, &mut native);
    assert!(app.active_turn_id().is_none());
}

#[test]
fn a_coalesced_native_result_completes_every_consumed_prompt_uuid() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "first");
    submitted(&mut app, &mut native, "second");
    let mut message = result("second");
    message["user_message_uuids"] = json!(["first", "second"]);
    sdk(&mut app, &mut native, message);
    idle(&mut app, &mut native);
    assert!(app.active_turn_id().is_none());
    for id in ["first", "second"] {
        assert_eq!(
            app.turn_status_by_id().get(id),
            Some(&TurnStatus::Completed)
        );
    }
}

#[test]
fn sdk_error_is_failed_even_when_an_interrupt_was_requested() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "first");
    sdk(
        &mut app,
        &mut native,
        json!({ "type": "assistant", "user_message_uuid": "first", "message": { "content": [] } }),
    );
    native.begin_interrupt();
    let mut error = result("first");
    error["is_error"] = json!(true);
    error["errors"] = json!(["Model failed"]);
    sdk(&mut app, &mut native, error);
    idle(&mut app, &mut native);
    assert_eq!(
        app.turn_status_by_id().get("first"),
        Some(&TurnStatus::Failed)
    );
}

#[test]
fn empty_sdk_errors_fall_back_to_native_result_text_then_a_useful_generic_error() {
    for (text, expected) in [
        ("Native failure detail", "Native failure detail"),
        (" \t\n", "Claude turn failed"),
    ] {
        let mut app = SessionState::new("session".into(), Default::default(), false);
        let mut native = FixtureNormalizer::default();
        app.enable_ralph_mode(RalphConfig {
            prompt_path: PathBuf::from("fixture"),
            base_prompt: "start".to_string(),
            continuation_prompt: "continue".to_string(),
            done_marker: "@@DONE@@".to_string(),
            blocked_marker: "@@BLOCKED@@".to_string(),
            waiting_marker: "@@WAITING@@".to_string(),
        });
        submitted(&mut app, &mut native, "first");
        let mut error = result("first");
        error["is_error"] = json!(true);
        error["errors"] = json!([]);
        error["result"] = json!(text);
        sdk(&mut app, &mut native, error);
        idle(&mut app, &mut native);
        assert_eq!(
            app.messages().last().expect("failure message").text,
            expected
        );
        assert_eq!(
            app.turn_status_by_id().get("first"),
            Some(&TurnStatus::Failed)
        );
        assert!(!app.ralph_enabled());
    }
}

#[test]
fn native_abort_reasons_remain_interrupts_even_if_the_result_has_an_error_flag() {
    for reason in ["aborted_streaming", "aborted_tools"] {
        let mut app = SessionState::new("session".into(), Default::default(), false);
        let mut native = FixtureNormalizer::default();
        submitted(&mut app, &mut native, "first");
        let mut message = result("first");
        message["terminal_reason"] = json!(reason);
        message["is_error"] = json!(true);
        sdk(&mut app, &mut native, message);
        idle(&mut app, &mut native);
        assert_eq!(
            app.turn_status_by_id().get("first"),
            Some(&TurnStatus::Interrupted)
        );
    }
}

#[test]
fn failed_interrupt_rolls_back_only_the_captured_prompt_ids() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
    submitted(&mut app, &mut native, "first");
    sdk(
        &mut app,
        &mut native,
        json!({"type":"assistant","user_message_uuid":"first","message":{"role":"assistant","content":[]}}),
    );
    let captured = native.begin_interrupt();
    assert_eq!(captured, ["first"]);
    native.rollback_interrupt(&captured);
    sdk(&mut app, &mut native, result("first"));
    idle(&mut app, &mut native);
    assert_eq!(
        app.turn_status_by_id().get("first"),
        Some(&TurnStatus::Completed)
    );
}

#[test]
fn failed_ralph_turn_discards_automatic_work_and_preserves_queued_human_input() {
    let mut app = SessionState::new("session".into(), Default::default(), false);
    let mut native = FixtureNormalizer::default();
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
    submitted(&mut app, &mut native, "first");
    let mut error = result("first");
    error["is_error"] = json!(true);
    sdk(&mut app, &mut native, error);
    idle(&mut app, &mut native);
    assert!(!app.ralph_enabled());
    let queued = app
        .dequeue_turn_input(Instant::now())
        .expect("human input retained");
    assert_eq!(queued.text, "human next");
    assert_eq!(queued.origin, PromptOrigin::Human);
    assert!(app.dequeue_turn_input(Instant::now()).is_none());
}
