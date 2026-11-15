//! Keep the original Claude weekly badge text at the frontend's plain formatter.

use carlos_core::session::usage::{RateWindow, WEEK_MINUTES};

use super::WeeklyIndicator;

#[test]
fn the_shared_weekly_window_keeps_its_original_sixty_percent_label() {
    let now = 1_800_000_000;
    let quota = RateWindow::new(40, Some(WEEK_MINUTES), Some(now + 86_400));
    let label = WeeklyIndicator::from(quota.indicator(now).expect("global weekly window"));
    assert_eq!(label.percent, "60%");
}
