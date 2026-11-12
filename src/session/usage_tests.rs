//! Check plain pace categories at percentage and clock boundaries.

use super::usage::{Pace, RateWindow, WEEK_MINUTES};

#[test]
fn pace_boundaries_include_equality_and_cap_time_at_one_week() {
    for (remaining, seconds, expected) in [
        (50, 302399, Pace::OnTrack),
        (50, 302400, Pace::OnTrack),
        (50, 302401, Pace::Close),
        (75, 604800, Pace::Close),
        (74, 604800, Pace::Behind),
        (100, 604801, Pace::OnTrack),
        (99, 604801, Pace::Close),
        (0, 1, Pace::Behind),
        (1, 1, Pace::OnTrack),
    ] {
        let quota = RateWindow::new(100 - remaining, Some(WEEK_MINUTES), Some(seconds));
        assert_eq!(quota.indicator(0).expect("future quota").pace(), expected);
    }
}

#[test]
fn reset_at_or_before_now_and_overflow_never_claim_a_new_allowance() {
    for (now, reset) in [(100, 99), (100, 100), (i64::MIN, i64::MAX)] {
        assert!(RateWindow::new(100, Some(WEEK_MINUTES), Some(reset))
            .indicator(now)
            .is_none());
    }
}
