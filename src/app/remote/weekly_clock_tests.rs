//! Keep the historical redraw schedules on each device's actual countdown clock.

use std::time::{Duration, Instant};

use super::StatusClock;

#[test]
fn combined_wakeups_keep_read_expiry_refresh_and_countdowns_for_every_quota_state() {
    let mut clock = StatusClock::default();
    let start = Instant::now();
    for (seconds, redraw, wait) in [
        (0, true, 60),
        (9, false, 51),
        (10, false, 50),
        (59, false, 1),
        (60, true, 60),
        (299, true, 60),
        (300, false, 59),
        (310, false, 49),
        (359, true, 60),
        (600, true, 60),
    ] {
        let now = start + Duration::from_secs(seconds);
        assert_eq!(clock.tick(now, 0, false), redraw, "at {seconds}");
        assert_eq!(
            clock
                .minute
                .expect("local deadline")
                .saturating_duration_since(now),
            Duration::from_secs(wait),
            "at {seconds}"
        );
    }
}

#[test]
fn claude_quota_display_keeps_its_original_minute_schedule() {
    let mut clock = StatusClock::default();
    let start = Instant::now();
    for (seconds, redraw, wait) in [
        (0, true, 60),
        (59, false, 1),
        (60, true, 60),
        (299, true, 60),
        (300, false, 59),
        (359, true, 60),
    ] {
        let now = start + Duration::from_secs(seconds);
        assert_eq!(clock.tick(now, 0, false), redraw, "at {seconds}");
        assert_eq!(
            clock
                .minute
                .expect("local deadline")
                .saturating_duration_since(now),
            Duration::from_secs(wait),
            "at {seconds}"
        );
    }
}
