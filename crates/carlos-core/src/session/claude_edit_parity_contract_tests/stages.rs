//! Retain each original native recovery owner across all five edit stages.

use std::time::Instant;

use carlos_proto::submissions::RecoveryKind;
use serde_json::json;

use super::{advance, core, event, notices, recovered};

#[test]
fn edit_failure_at_every_stage_recovers_once_and_never_replaces_newer_typing() {
    for stage in 0..5 {
        let mut core = core();
        advance(&mut core, stage);
        let mut update = Default::default();
        core.fail_claude(&mut update);
        assert!(core.claude_edit_progress().is_none());
        let (kind, prefix) = if stage == 2 {
            (RecoveryKind::Queued, "queued")
        } else {
            (RecoveryKind::Edit, "edited")
        };
        assert_eq!(recovered(&update), [("edited text", kind)], "stage {stage}");
        assert_eq!(
            notices(&core),
            [format!("Recovered unsent {prefix} prompt:\nedited text")]
        );
        let mut duplicate = Default::default();
        core.fail_claude(&mut duplicate);
        assert!(recovered(&duplicate).is_empty());
    }
}

#[test]
fn native_reset_at_every_edit_stage_drops_progress_without_inventing_recovery() {
    for stage in 0..5 {
        let mut core = core();
        advance(&mut core, stage);
        let update = event(
            &mut core,
            "claude/conversationReset",
            json!({"new_conversation_id":"reset","user_message_uuid":"clear"}),
        );
        assert!(core.claude_edit_progress().is_none());
        assert!(core.claude_edit_prompt_for_test().is_none());
        assert!(recovered(&update).is_empty());
        let queued = core.dequeue_turn_input(Instant::now());
        assert_eq!(
            queued.as_ref().map(|input| input.text.as_str()),
            (stage == 2).then_some("edited text")
        );
        assert!(core.dequeue_turn_input(Instant::now()).is_none());
    }
}

#[test]
fn switch_at_every_stage_carries_only_saved_branch_text_and_retains_queued_humans() {
    for stage in 0..5 {
        let mut core = core();
        advance(&mut core, stage);
        let mut update = Default::default();
        core.switch_claude_session(3, "other", &mut update);
        assert_eq!(core.claude_edit_progress().is_some(), stage < 2);
        assert_eq!(
            recovered(&update),
            if stage == 2 {
                vec![("edited text", RecoveryKind::Queued)]
            } else {
                Vec::new()
            }
        );
        core.receive_claude_capabilities(&json!({"models":[]}), &mut Default::default());
        let next = core.dequeue_turn_input(Instant::now());
        assert_eq!(
            next.as_ref().map(|input| input.text.as_str()),
            (stage < 2).then_some("edited text")
        );
        assert!(core.dequeue_turn_input(Instant::now()).is_none());
    }
}
