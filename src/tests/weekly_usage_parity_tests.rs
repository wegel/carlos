//! Pin quota text, colors and every timer boundary against the previous app.

use std::cell::Cell;
use std::time::{Duration, Instant};

use carlos_core::session::usage::RateWindow;
use serde_json::json;

use super::tests::QuotaClient;
use crate::app::state::AppState;
use crate::app::weekly_usage::WeeklyIndicator;
use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

const NOW: i64 = 1_800_000_000;

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

#[test]
fn combined_wakeups_keep_read_expiry_refresh_and_countdowns_for_every_quota_state() {
    for quota in [
        None,
        Some((40, 86400)),
        Some((100, 86400)),
        Some((40, -1)),
        Some((40, 0)),
    ] {
        let client = QuotaClient {
            reads: Cell::new(0),
        };
        let mut app = AppState::new("thread".into());
        let start = Instant::now();
        if let Some((used, seconds)) = quota {
            app.session
                .update_rate_limits(&json!({"secondary":{"usedPercent":used,
                "windowDurationMins":10080,"resetsAt":NOW+seconds}}));
        }
        assert_eq!(app.rate_limits_wait(start), None);
        for (seconds, before, redraw, after, reads) in [
            (0, None, true, 10, 1),
            (9, Some(1), false, 1, 1),
            (10, Some(0), false, 50, 1),
            (59, Some(1), false, 1, 1),
            (60, Some(0), true, 60, 1),
            (299, Some(0), true, 1, 1),
            (300, Some(0), false, 10, 2),
            (310, Some(0), false, 49, 2),
            (359, Some(0), true, 60, 2),
            (600, Some(0), true, 10, 3),
        ] {
            let now = start + Duration::from_secs(seconds);
            assert_eq!(
                app.rate_limits_wait(now),
                before.map(Duration::from_secs),
                "{quota:?} at {seconds}"
            );
            assert_eq!(
                app.tick_rate_limits(&client, now),
                redraw,
                "{quota:?} at {seconds}"
            );
            assert_eq!(
                app.rate_limits_wait(now),
                Some(Duration::from_secs(after)),
                "{quota:?} at {seconds}"
            );
            assert_eq!(client.reads.get(), reads);
        }
    }
}

#[test]
fn absent_quota_clears_only_on_a_successful_snapshot_not_an_error_or_sparse_update() {
    for result in [
        json!({}),
        json!({"rateLimits":{}}),
        json!({"rateLimitsByLimitId":{"other":{}}}),
    ] {
        let client = QuotaClient {
            reads: Cell::new(0),
        };
        let mut app = AppState::new("thread".into());
        let start = Instant::now();
        app.session
            .update_rate_limits(&json!({"secondary":{"usedPercent":50,
            "windowDurationMins":10080,"resetsAt":NOW+86400}}));
        app.tick_rate_limits(&client, start);
        app.session.update_rate_limits(&json!({"secondary":null}));
        assert!(app
            .session
            .rate_limits_response(&json!({"id":"read-1","error":{"code":-1}})));
        assert_eq!(
            app.session
                .rate_limits()
                .indicator_at(NOW)
                .expect("kept on error")
                .remaining(),
            50
        );
        app.tick_rate_limits(&client, start + Duration::from_secs(300));
        assert!(app
            .session
            .rate_limits_response(&json!({"id":"read-2","result":result})));
        assert!(app.session.rate_limits().indicator_at(NOW).is_none());
        assert_eq!(
            app.rate_limits_wait(start + Duration::from_secs(300)),
            Some(Duration::from_secs(60))
        );
        app.clear_rate_limits();
        assert_eq!(app.rate_limits_wait(start + Duration::from_secs(300)), None);
    }
}
