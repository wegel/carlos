//! Keep exact weekly percentages, countdown text and pace colors at every recorded boundary.

use carlos_core::session::usage::{RateWindow, WEEK_MINUTES};
use serde_json::{json, Value};

use crate::app::weekly_usage::WeeklyIndicator;
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

const NOW: i64 = 1_800_000_000;

fn window(used: u64, days: f64) -> Value {
    json!({"usedPercent":used,"windowDurationMins":WEEK_MINUTES,
        "resetsAt":NOW+(days*86_400.0) as i64})
}

#[test]
fn weekly_pace_compares_allowance_left_with_time_left() {
    for (used, days, hue) in [
        (60, 2.1, COLOR_GUTTER_USER),
        (60, 3.5, COLOR_TOOL_EDIT),
        (60, 6.0, COLOR_DIFF_REMOVE),
        (100, 0.01, COLOR_DIFF_REMOVE),
        (0, 7.0, COLOR_GUTTER_USER),
    ] {
        let quota = RateWindow::merge(None, &window(used, days)).expect("weekly window");
        let label = WeeklyIndicator::from(quota.indicator(NOW).expect("current weekly quota"));
        assert_eq!(label.percent, format!("{}%", 100 - used));
        assert_eq!(label.countdown, format!("{days:.1}d"));
        assert_eq!(label.color, hue);
    }
}

#[test]
fn weekly_text_and_colors_keep_the_exact_pace_boundaries() {
    for (used, seconds, percent, countdown, color) in [
        (0, 604800, "100%", "7.0d", COLOR_GUTTER_USER),
        (50, 302400, "50%", "3.5d", COLOR_GUTTER_USER),
        (51, 302400, "49%", "3.5d", COLOR_TOOL_EDIT),
        (25, 604800, "75%", "7.0d", COLOR_TOOL_EDIT),
        (26, 604800, "74%", "7.0d", COLOR_DIFF_REMOVE),
        (100, 1, "0%", "0.0d", COLOR_DIFF_REMOVE),
        (99, 1, "1%", "0.0d", COLOR_GUTTER_USER),
        (0, 1209600, "100%", "14.0d", COLOR_GUTTER_USER),
        (1, 1209600, "99%", "14.0d", COLOR_TOOL_EDIT),
        (101, 86400, "0%", "1.0d", COLOR_DIFF_REMOVE),
    ] {
        let window = RateWindow::merge(
            None,
            &json!({"usedPercent":used,
            "windowDurationMins":10080,"resetsAt":NOW+seconds}),
        )
        .expect("window");
        let actual = WeeklyIndicator::from(window.indicator(NOW).expect("unexpired"));
        assert_eq!(
            (
                actual.percent.as_str(),
                actual.countdown.as_str(),
                actual.color
            ),
            (percent, countdown, color),
            "used={used}, seconds={seconds}"
        );
    }
}
