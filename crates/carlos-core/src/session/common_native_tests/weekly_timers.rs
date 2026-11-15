//! Keep native quota expiry and refresh deadlines separate from each device's countdown.

use std::cell::Cell;
use std::time::{Duration, Instant};

use serde_json::json;

use super::session;
use super::weekly_test_support::{QuotaClient, NOW};

#[test]
fn refreshes_are_bounded_and_idle_countdowns_keep_waking() {
    let client = QuotaClient {
        reads: Cell::new(0),
    };
    let now = Instant::now();
    let mut app = session("thread");
    app.tick_rate_limits(&client, now);
    assert_eq!(client.reads.get(), 1);
    assert_eq!(app.rate_limits().wait(now), Some(Duration::from_secs(10)));
    app.tick_rate_limits(&client, now + Duration::from_secs(10));
    assert!(!app.rate_limits_response(&json!({"id":"read-1","result":{}})));
    assert_eq!(
        app.rate_limits().wait(now + Duration::from_secs(10)),
        Some(Duration::from_secs(290))
    );
    app.tick_rate_limits(&client, now + Duration::from_secs(60));
    assert_eq!(client.reads.get(), 1);
    app.tick_rate_limits(&client, now + Duration::from_secs(300));
    assert_eq!(client.reads.get(), 2);
    assert!(!app.rate_limits_response(&json!({"id":"read-1","result":{}})));
    assert!(app.rate_limits_response(&json!({"id":"read-2","error":{"code":-32601}})));
    assert!(app.rate_limits().indicator_at(NOW).is_none());
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
        let mut app = session("thread");
        let start = Instant::now();
        if let Some((used, seconds)) = quota {
            app.update_rate_limits(&json!({"secondary":{"usedPercent":used,
                "windowDurationMins":10080,"resetsAt":NOW+seconds}}));
        }
        assert_eq!(app.rate_limits().wait(start), None);
        for (seconds, before, after, reads) in [
            (0, None, 10, 1),
            (9, Some(1), 1, 1),
            (10, Some(0), 290, 1),
            (59, Some(241), 241, 1),
            (60, Some(240), 240, 1),
            (299, Some(1), 1, 1),
            (300, Some(0), 10, 2),
            (310, Some(0), 290, 2),
            (359, Some(241), 241, 2),
            (600, Some(0), 10, 3),
        ] {
            let now = start + Duration::from_secs(seconds);
            assert_eq!(
                app.rate_limits().wait(now),
                before.map(Duration::from_secs),
                "{quota:?} at {seconds}"
            );
            app.tick_rate_limits(&client, now);
            assert_eq!(
                app.rate_limits().wait(now),
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
        let mut app = session("thread");
        let start = Instant::now();
        app.update_rate_limits(&json!({"secondary":{"usedPercent":50,
            "windowDurationMins":10080,"resetsAt":NOW+86400}}));
        app.tick_rate_limits(&client, start);
        app.update_rate_limits(&json!({"secondary":null}));
        assert!(app.rate_limits_response(&json!({"id":"read-1","error":{"code":-1}})));
        assert_eq!(
            app.rate_limits()
                .indicator_at(NOW)
                .expect("kept on error")
                .remaining(),
            50
        );
        app.tick_rate_limits(&client, start + Duration::from_secs(300));
        assert!(app.rate_limits_response(&json!({"id":"read-2","result":result})));
        assert!(app.rate_limits().indicator_at(NOW).is_none());
        assert_eq!(
            app.rate_limits().wait(start + Duration::from_secs(300)),
            Some(Duration::from_secs(300))
        );
        app.clear_rate_limits();
        assert_eq!(
            app.rate_limits().wait(start + Duration::from_secs(300)),
            None
        );
    }
}
