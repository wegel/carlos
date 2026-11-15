//! Bind only the exact dispatched human text and original returned native UUID.

use carlos_proto::submissions::RecoveryKind;
use serde_json::json;

use super::{accepted, advance, core, event, failed, notices, recovered};
use crate::backend::PromptOrigin;

#[test]
fn bind_requires_matching_dispatched_human_and_uses_nested_then_flat_uuid() {
    for response in [
        json!({"result":{"turn":{"id":"nested"}},"turn":{"id":"flat"}}),
        json!({"result":{"turn":{"id":3}},"turn":{"id":"nested"}}),
    ] {
        let mut core = core();
        advance(&mut core, 2);
        core.bind_claude_edit("edited text", PromptOrigin::Human, None, &response);
        accepted(&mut core, "nested");
        assert!(
            core.claude_edit_progress().is_some(),
            "queued text cannot bind"
        );
        core.dispatch_claude_edit("edited text", PromptOrigin::Automatic, None);
        core.dispatch_claude_edit("other", PromptOrigin::Human, None);
        core.bind_claude_edit("edited text", PromptOrigin::Human, None, &response);
        accepted(&mut core, "nested");
        assert!(core.claude_edit_progress().is_some());
        core.dispatch_claude_edit("edited text", PromptOrigin::Human, None);
        core.bind_claude_edit("other", PromptOrigin::Human, None, &response);
        core.bind_claude_edit("edited text", PromptOrigin::Automatic, None, &response);
        accepted(&mut core, "nested");
        assert!(core.claude_edit_progress().is_some());
        core.bind_claude_edit("edited text", PromptOrigin::Human, None, &response);
        accepted(&mut core, "flat");
        assert!(core.claude_edit_progress().is_some());
        accepted(&mut core, "nested");
        assert!(core.claude_edit_progress().is_none());
        assert_eq!(notices(&core), ["Claude started the edited branch"]);
        accepted(&mut core, "nested");
        assert_eq!(notices(&core).len(), 1);
    }
}

#[test]
fn async_fork_and_start_failures_keep_exact_recovery_and_failure_wording() {
    for (stage, method, reason) in [
        (0, "claude/forkFailed", "Could not fork Claude session"),
        (
            1,
            "claude/sessionStartFailed",
            "Claude could not start the edited branch",
        ),
    ] {
        let mut core = core();
        advance(&mut core, stage);
        let update = event(&mut core, method, json!({}));
        assert_eq!(recovered(&update), [("edited text", RecoveryKind::Edit)]);
        assert_eq!(
            notices(&core),
            [
                "Recovered unsent edited prompt:\nedited text".into(),
                format!("Could not edit Claude message: {reason}. Draft kept.")
            ]
        );
        assert!(!core.reject_claude_edit_send("unrelated", None));
    }
}

#[test]
fn recovery_keeps_identical_composer_and_unrelated_ack_cannot_clear_its_failure() {
    let mut core = core();
    advance(&mut core, 0);
    let update = failed(&mut core, "unavailable");
    assert_eq!(recovered(&update), [("edited text", RecoveryKind::Edit)]);
    assert_eq!(
        notices(&core),
        [
            "Recovered unsent edited prompt:\nedited text",
            "Could not edit Claude message: unavailable. Draft kept."
        ]
    );
    assert!(accepted(&mut core, "later").effects.is_empty());
    assert_eq!(notices(&core).len(), 2);
}

#[test]
fn missing_uuid_unbinds_a_dispatched_edit_and_same_text_is_still_the_send_failure_match() {
    let mut core = core();
    advance(&mut core, 4);
    core.bind_claude_edit("edited text", PromptOrigin::Human, None, &json!({}));
    accepted(&mut core, "edit-uuid");
    assert!(core.claude_edit_progress().is_some());
    assert!(!core.reject_claude_edit_send("other", None));
    assert!(core.reject_claude_edit_send("edited text", None));
    let update = failed(&mut core, "fixed failure");
    assert!(core.claude_edit_progress().is_none());
    assert_eq!(recovered(&update), [("edited text", RecoveryKind::Edit)]);
    assert_eq!(
        notices(&core),
        [
            "Recovered unsent edited prompt:\nedited text",
            "Could not edit Claude message: fixed failure. Draft kept."
        ]
    );
}
