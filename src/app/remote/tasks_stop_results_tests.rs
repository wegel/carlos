//! Keep private rejected and pending stop evidence after native completion or replacement.

use carlos_proto::submissions::OperationState;

use super::{live_notice, Notice};

#[test]
fn rejected_stops_remain_visible_after_completion_replacement_and_row_removal() {
    let old = "a".repeat(64).parse().expect("original run");
    let new = "b".repeat(64).parse().expect("replacement run");
    let notice = Notice {
        task: "c".repeat(64).parse().expect("original task"),
        run: old,
        text: "Stop failed: original rejection".into(),
        rejected: true,
    };
    assert_eq!(
        notice.for_run(Some(&notice.run), true).as_deref(),
        Some("Stop failed: original rejection")
    );
    assert_eq!(
        notice.for_run(Some(&new), false).as_deref(),
        Some("Prior-run: Stop failed: original rejection")
    );
    assert_eq!(
        notice.for_run(None, true).as_deref(),
        Some("Original run: Stop failed: original rejection")
    );
}

#[test]
fn native_completion_retires_only_a_matching_accepted_stop_notice() {
    let run = "a".repeat(64).parse().expect("original run");
    let notice = Notice {
        task: "c".repeat(64).parse().expect("task"),
        run,
        text: "Stop requested".into(),
        rejected: false,
    };
    assert!(notice.for_run(Some(&notice.run), false).is_some());
    assert!(notice.for_run(Some(&notice.run), true).is_none());
    assert!(notice.for_run(None, true).is_none());
}

#[test]
fn pending_and_uncertain_original_stops_keep_their_prior_run_label() {
    assert_eq!(
        live_notice(&OperationState::Pending, true, true).as_deref(),
        Some("Prior-run: Stop pending; waiting for original native acknowledgement")
    );
    assert_eq!(
        live_notice(&OperationState::Queued, true, false).as_deref(),
        Some("Prior-run: Stop pending; waiting for accepted connection. No automatic retry")
    );
    let state = OperationState::Uncertain {
        message: "native response missing".into(),
        recovery: None,
    };
    assert_eq!(
        live_notice(&state, true, true).as_deref(),
        Some("Prior-run: Stop failed: native response missing. stop uncertain; No automatic retry")
    );
}
