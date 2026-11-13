//! Reject unsafe output shapes and cancel a hung executable within its deadline.

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::summaries::provider::validate;
use crate::summaries::{process, Control};

#[test]
fn label_validation_rejects_controls_multiline_and_unbounded_text() {
    for text in ["", "first\nsecond", "\x1b[31mred", "```source```"] {
        assert!(validate(text).is_err());
    }
    assert!(validate(&"long ".repeat(30)).is_err());
    assert_eq!(
        validate("  Delete old screenshots  ").expect("label"),
        "Delete old screenshots"
    );
}

#[cfg(unix)]
#[test]
fn a_child_that_does_not_read_stdin_still_times_out() {
    let control = Arc::new(Control {
        enabled: AtomicBool::new(true),
        stopped: AtomicBool::new(false),
        pid: AtomicU32::new(0),
        epoch: AtomicU64::new(0),
    });
    let start = Instant::now();
    let result = process::run(
        Command::new("sh").args(["-c", "sleep 30"]),
        "x".repeat(131_072),
        Duration::from_millis(100),
        &control,
        0,
    );
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[cfg(unix)]
#[test]
fn work_from_before_disable_cannot_start_after_reenable() {
    let control = Arc::new(Control {
        enabled: AtomicBool::new(true),
        stopped: AtomicBool::new(false),
        pid: AtomicU32::new(0),
        epoch: AtomicU64::new(1),
    });
    let result = process::run(
        Command::new("sh").args(["-c", "exit 0"]),
        String::new(),
        Duration::from_secs(1),
        &control,
        0,
    );
    assert!(result.is_err());
    assert_eq!(control.pid.load(std::sync::atomic::Ordering::Relaxed), 0);
}
