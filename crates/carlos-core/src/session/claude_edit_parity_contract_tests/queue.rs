//! Retire only native automatic work and keep accepted humans in their original order.

use carlos_proto::submissions::RecoveryKind;

use super::{core, notices, recovered};
use crate::backend::PromptOrigin;

#[test]
fn queued_recovery_preserves_human_order_and_discards_automatic_continuations() {
    let mut core = core();
    for (text, origin) in [
        ("first", PromptOrigin::Human),
        ("automatic", PromptOrigin::Automatic),
        ("second", PromptOrigin::Human),
    ] {
        core.ralph_enqueue_prompt(text.into(), true, origin);
        core.record_prompt_history(text, None);
    }
    core.queue_ralph_continuation("delayed automatic");
    let mut update = Default::default();
    core.retire_claude_queue(&mut update);
    assert_eq!(
        notices(&core),
        [
            "Recovered unsent queued prompt:\nfirst",
            "Recovered unsent queued prompt:\nsecond"
        ]
    );
    assert_eq!(
        recovered(&update),
        [
            ("first", RecoveryKind::Queued),
            ("second", RecoveryKind::Queued)
        ]
    );
    assert!(core.queued_turn_inputs_is_empty());
    assert!(!core.has_pending_ralph_continuation());
}

#[test]
fn due_ralph_continuations_stay_behind_humans_and_keep_their_actual_sources() {
    let mut core = core();
    core.queue_ralph_continuation("automatic");
    let due = core
        .ralph_pending_continuation_deadline()
        .expect("deadline");
    for text in ["first human", "second human"] {
        core.queue_turn_input(text);
    }
    for (text, origin) in [
        ("first human", PromptOrigin::Human),
        ("second human", PromptOrigin::Human),
        ("automatic", PromptOrigin::Automatic),
    ] {
        let input = core.dequeue_turn_input(due).expect("ordered prompt");
        assert_eq!(input.text, text);
        assert!(input.origin == origin);
    }
    assert!(core.dequeue_turn_input(due).is_none());
}
