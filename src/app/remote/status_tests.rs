//! Keep each terminal's countdown and animation clocks independent of backend polling.

use std::time::{Duration, Instant};

use carlos_proto::bytes::ByteLength;
use carlos_proto::status::MainActivity;
use carlos_proto::task_overview::{TaskLabel, TaskMainState};

use super::{activity_label, StatusClock};

#[test]
fn countdown_clocks_advance_locally_and_one_device_cannot_move_anothers_deadline() {
    let now = Instant::now();
    let mut first = StatusClock::default();
    let mut second = StatusClock::default();
    assert!(first.tick(now, 0, false));
    assert!(!first.tick(now + Duration::from_secs(59), 1, false));
    assert!(second.tick(now + Duration::from_secs(59), 1, false));
    assert!(first.tick(now + Duration::from_secs(60), 2, false));
    assert!(!second.tick(now + Duration::from_secs(60), 2, false));
    assert!(!first.tick(now + Duration::from_secs(119), 3, false));
    assert!(second.tick(now + Duration::from_secs(119), 3, false));
}

#[test]
fn activity_sweeps_redraw_only_once_per_animation_step_while_work_is_running() {
    let now = Instant::now();
    let mut clock = StatusClock::default();
    assert!(clock.tick(now, 0, false));
    assert!(clock.tick(now, 1, true));
    assert!(!clock.tick(now, 1, true));
    assert!(!clock.tick(now, 2, false));
    assert!(clock.tick(now, 3, true));
    assert!(!clock.tick(now, 4, false));
}

#[test]
fn native_transient_then_thinking_then_process_state_keep_the_existing_label_order() {
    let mut activity = MainActivity {
        state: TaskMainState::Running,
        thinking_tokens: Some(43),
        transient: Some(TaskLabel {
            text: "Requesting".into(),
            bytes: ByteLength::new(10),
            complete: true,
        }),
    };
    assert_eq!(activity_label(&activity).as_deref(), Some("Requesting"));
    activity.transient = None;
    assert_eq!(
        activity_label(&activity).as_deref(),
        Some("Thinking ~43 tokens")
    );
    activity.thinking_tokens = None;
    assert_eq!(activity_label(&activity).as_deref(), Some("Running"));
    activity.state = TaskMainState::RequiresAction;
    assert_eq!(
        activity_label(&activity).as_deref(),
        Some("Requires action")
    );
    activity.state = TaskMainState::Idle;
    assert_eq!(activity_label(&activity), None);
}

#[path = "status_footer_tests.rs"]
mod footer;
