//! Recover only original native edits and keep their history and newest failure reason.

use std::time::Instant;

use carlos_proto::submissions::RecoveryKind;
use serde_json::json;

use super::{begin, core};
use crate::backend::PromptOrigin;
use crate::claude_backend::FixtureNormalizer;
use crate::session::claude::ClaudePhase;
use crate::session::claude_edit_parity_contract_tests::send::{human, submit};
use crate::session::claude_edit_parity_contract_tests::{failed, recovered as recovered_update};
use crate::session::claude_session_contract_tests::{recovered, send};
use crate::session::models::Role;

#[test]
fn failed_fork_keeps_newer_composer_and_recovers_saved_edit_without_erasing_old_history() {
    let mut core = core();
    let mut native = FixtureNormalizer::default();
    begin(&mut core, "saved edit 日本語");
    let outcome = send(
        &mut core,
        &mut native,
        "claude/forkFailed",
        1,
        json!({"message":"The exact selected UUID is unavailable"}),
    );
    assert_eq!(core.claude().phase(), ClaudePhase::Ready);
    assert_eq!(core.thread_id(), "original");
    assert!(core
        .messages()
        .iter()
        .any(|message| message.text == "Retained row 79"));
    assert!(core
        .messages()
        .iter()
        .any(|message| message.text.contains("exact selected UUID")));
    assert_eq!(
        recovered(&outcome),
        [("saved edit 日本語".into(), RecoveryKind::Edit, None)]
    );
}

#[test]
fn closed_helper_cannot_be_revived_by_late_fork_failure_or_old_generation_errors() {
    let mut core = core();
    let mut native = FixtureNormalizer::default();
    begin(&mut core, "recover closed edit");
    let closed = send(
        &mut core,
        &mut native,
        "claude/controller",
        1,
        json!({"frame":{"type":"event","event":"closed","reason":"timeout"}}),
    );
    assert_eq!(
        recovered(&closed),
        [("recover closed edit".into(), RecoveryKind::Edit, None)]
    );
    let late = send(
        &mut core,
        &mut native,
        "claude/forkFailed",
        1,
        json!({"message":"late timeout"}),
    );
    assert!(recovered(&late).is_empty());
    assert_eq!(core.claude().phase(), ClaudePhase::Closed);
    send(
        &mut core,
        &mut native,
        "claude/sessionChanged",
        2,
        json!({"sessionId":"branch"}),
    );
    send(
        &mut core,
        &mut native,
        "claude/forkFailed",
        1,
        json!({"message":"old branch failure"}),
    );
    assert_eq!(core.claude().phase(), ClaudePhase::Starting);
    assert_eq!(core.thread_id(), "branch");
    assert!(!core
        .messages()
        .iter()
        .any(|message| message.text.contains("old branch failure")));
}

#[test]
fn failed_dequeued_edit_does_not_replace_newer_composer_or_submit_recovery_automatically() {
    let mut core = core();
    core.queue_claude_edit("old edit");
    core.dispatch_claude_edit("old edit", PromptOrigin::Human, None);
    assert!(core.reject_claude_edit_send("old edit", None));
    let update = failed(&mut core, "prompt rejected");
    assert!(core.claude_edit_progress().is_none());
    assert_eq!(
        recovered_update(&update),
        [("old edit", RecoveryKind::Edit)]
    );
    assert!(core.dequeue_turn_input(Instant::now()).is_none());
}

#[test]
fn close_recovers_sending_edits_once_whether_the_queue_still_owns_them_or_dispatch_started() {
    for queued in [false, true] {
        let mut core = core();
        core.queue_claude_edit("saved outgoing edit");
        if queued {
            core.queue_turn_input_with_history("saved outgoing edit", true);
        } else {
            core.dispatch_claude_edit("saved outgoing edit", PromptOrigin::Human, None);
        }
        let mut update = Default::default();
        core.fail_claude(&mut update);
        assert_eq!(core.claude().phase(), ClaudePhase::Closed);
        assert_eq!(
            recovered_update(&update),
            [(
                "saved outgoing edit",
                if queued {
                    RecoveryKind::Queued
                } else {
                    RecoveryKind::Edit
                }
            )]
        );
        let mut duplicate = Default::default();
        core.fail_claude(&mut duplicate);
        assert!(
            recovered_update(&duplicate).is_empty(),
            "one edit has one recovery owner"
        );
    }
}

#[test]
fn branch_start_recovers_old_and_late_humans_and_never_replays_automatic_work_before_the_edit() {
    let mut core = core();
    let mut native = FixtureNormalizer::default();
    core.queue_turn_input_with_history("saved edit", true);
    core.ralph_enqueue_prompt("old automatic".into(), false, PromptOrigin::Automatic);
    let old = begin(&mut core, "saved edit");
    assert_eq!(
        recovered_update(&old),
        [("saved edit", RecoveryKind::Queued)]
    );
    send(
        &mut core,
        &mut native,
        "claude/sessionChanged",
        2,
        json!({"sessionId":"branch"}),
    );
    core.queue_turn_input_with_history("late startup human", true);
    let late = send(
        &mut core,
        &mut native,
        "claude/controller",
        2,
        json!({"requestMethod":"start","frame":{"type":"reply","ok":true,"result":{"models":[]}}}),
    );
    let saved = core
        .dequeue_turn_input(Instant::now())
        .expect("saved edit only");
    assert_eq!(saved.text, "saved edit");
    assert!(saved.origin == PromptOrigin::Human);
    assert!(core.dequeue_turn_input(Instant::now()).is_none());
    assert_eq!(
        recovered(&late),
        [("late startup human".into(), RecoveryKind::Queued, None)]
    );
}

#[test]
fn a_successful_later_send_dismisses_the_failure_badge_but_keeps_saved_drafts() {
    let mut core = core();
    begin(&mut core, "saved failed edit");
    let failure = failed(&mut core, "fork unavailable");
    let (accepted, trace) = submit(&mut core, "later composer", human(false), false, false);
    assert_eq!(trace.len(), 1);
    assert!(core.claude_edit_progress().is_none());
    assert!(recovered_update(&accepted.update.changes).is_empty());
    assert_eq!(
        recovered_update(&failure),
        [("saved failed edit", RecoveryKind::Edit)]
    );
}

#[test]
fn a_later_failed_attempt_adds_its_specific_reason_at_the_current_transcript_tail() {
    let mut core = core();
    begin(&mut core, "first edit");
    failed(&mut core, "OLD_REASON");
    core.append_message(Role::Assistant, "later normal chat");
    begin(&mut core, "second edit");
    failed(&mut core, "NEW_REASON");
    assert!(core
        .messages()
        .last()
        .expect("current failure")
        .text
        .contains("NEW_REASON"));
    assert_eq!(
        core.messages()
            .iter()
            .filter(|row| row.text.contains("OLD_REASON"))
            .count(),
        1
    );
}
