//! Preserve private native recovery source, operation, order and once-only delivery.

use std::time::Instant;

use carlos_proto::submissions::{RecoveryKind, SubmittedText};
use serde_json::json;

use super::{core, frame, operation, recovered, send, switch};
use crate::claude_backend::FixtureNormalizer;
use crate::session::claude::ClaudePhase;

#[test]
fn closed_query_ignores_late_success_and_only_new_generation_can_reopen() {
    let mut app = core("original");
    let mut native = FixtureNormalizer::default();
    app.begin_claude_edit("saved edit".into());
    let outcomes = frame(
        &mut app,
        &mut native,
        1,
        json!({"type":"event","event":"closed","reason":"sdk_eof"}),
        None,
    );
    assert_eq!(
        recovered(&outcomes),
        [("saved edit".into(), RecoveryKind::Edit, None)]
    );
    for method in ["start", "capabilities"] {
        let outcomes = frame(
            &mut app,
            &mut native,
            1,
            json!({"type":"reply","ok":true,"result":{"models":[]}}),
            Some(method),
        );
        assert!(recovered(&outcomes).is_empty());
    }
    assert_eq!(app.claude().phase(), ClaudePhase::Closed);
    assert!(app.claude_edit_prompt_for_test().is_none());
    assert!(recovered(&switch(&mut app, &mut native)).is_empty());
    assert_eq!(app.claude().phase(), ClaudePhase::Starting);
}

#[test]
fn failures_before_and_after_switch_keep_the_correct_session_and_edit() {
    let mut app = core("original");
    let mut native = FixtureNormalizer::default();
    app.begin_claude_edit("edited draft".into());
    let before = send(
        &mut app,
        &mut native,
        "claude/forkFailed",
        1,
        json!({"message":"source unavailable"}),
    );
    assert_eq!(app.thread_id(), "original");
    assert_eq!(app.claude().phase(), ClaudePhase::Ready);
    assert_eq!(
        recovered(&before),
        [("edited draft".into(), RecoveryKind::Edit, None)]
    );
    app.begin_claude_edit("edited draft".into());
    switch(&mut app, &mut native);
    let after = send(
        &mut app,
        &mut native,
        "claude/sessionStartFailed",
        2,
        json!({"message":"helper input closed"}),
    );
    assert_eq!(app.claude().phase(), ClaudePhase::Closed);
    assert_eq!(
        recovered(&after),
        [("edited draft".into(), RecoveryKind::Edit, None)]
    );
}

#[test]
fn history_or_start_reply_failure_restores_the_edit_without_forging_a_turn() {
    for method in ["sessions.info", "sessions.read", "start"] {
        let mut app = core("session");
        let mut native = FixtureNormalizer::default();
        app.begin_claude_edit("recover me".into());
        let outcomes = frame(
            &mut app,
            &mut native,
            1,
            json!({"type":"reply","ok":false,"error":{"message":"Fixture failure"}}),
            Some(method),
        );
        assert_eq!(app.claude().phase(), ClaudePhase::Closed);
        assert_eq!(
            recovered(&outcomes),
            [("recover me".into(), RecoveryKind::Edit, None)]
        );
        assert!(app.active_turn_id().is_none());
    }
}

#[test]
fn closed_startup_recovers_queued_humans_without_replacing_the_new_composer() {
    let mut app = core("session");
    let mut native = FixtureNormalizer::default();
    app.set_claude_phase_for_test(ClaudePhase::Starting);
    let first = operation("1");
    let second = operation("2");
    for (text, operation) in [
        ("first human", first.clone()),
        ("second human", second.clone()),
    ] {
        app.queue_submitted_input(
            SubmittedText {
                text: text.into(),
                operation: Some(operation),
            },
            true,
        );
    }
    app.queue_ralph_continuation("automatic should disappear");
    let outcomes = frame(
        &mut app,
        &mut native,
        1,
        json!({"type":"event","event":"closed","reason":"start_failed"}),
        None,
    );
    assert_eq!(
        recovered(&outcomes),
        [
            ("first human".into(), RecoveryKind::Queued, Some(first)),
            ("second human".into(), RecoveryKind::Queued, Some(second))
        ]
    );
    assert!(app.dequeue_turn_input(Instant::now()).is_none());
    let duplicate = frame(
        &mut app,
        &mut native,
        1,
        json!({"type":"event","event":"closed","reason":"start_failed"}),
        None,
    );
    assert!(recovered(&duplicate).is_empty());
}

#[test]
fn failed_edit_with_newer_composer_can_be_recovered_once_without_overwriting_it() {
    let mut app = core("original");
    let mut native = FixtureNormalizer::default();
    let operation = operation("3");
    app.begin_claude_edit_for("edited before startup".into(), Some(operation.clone()));
    switch(&mut app, &mut native);
    let failed = send(
        &mut app,
        &mut native,
        "claude/sessionStartFailed",
        2,
        json!({"message":"startup failed","operation":operation}),
    );
    let closed = frame(
        &mut app,
        &mut native,
        2,
        json!({"type":"event","event":"closed","reason":"start_failed"}),
        None,
    );
    assert_eq!(
        recovered(&failed),
        [(
            "edited before startup".into(),
            RecoveryKind::Edit,
            Some(operation)
        )]
    );
    assert!(recovered(&closed).is_empty());
    assert!(app.claude_edit_prompt_for_test().is_none());
    assert!(app.dequeue_turn_input(Instant::now()).is_none());
}
